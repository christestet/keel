use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

fn main() {
    let root = repository_root();
    let files = markdown_files(&root);
    let known: BTreeSet<PathBuf> = files.iter().cloned().collect();
    let anchors: BTreeMap<_, _> = files
        .iter()
        .map(|file| {
            let source = fs::read_to_string(root.join(file))
                .unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", file.display())));
            (file.clone(), heading_anchors(&source))
        })
        .collect();
    let mut edges: BTreeMap<PathBuf, BTreeSet<PathBuf>> = BTreeMap::new();
    let mut errors = Vec::new();

    for file in &files {
        let source = fs::read_to_string(root.join(file))
            .unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", file.display())));
        let mut outgoing = BTreeSet::new();

        for (line, target) in local_links(&source) {
            let Some((link_path, fragment)) = local_target(&target) else {
                continue;
            };
            let resolved = if let Some(path) = &link_path {
                let Some(resolved) = normalize(file.parent().unwrap_or(Path::new("")), path) else {
                    errors.push(format!(
                        "{}:{line}: local link `{target}` escapes the repository",
                        file.display()
                    ));
                    continue;
                };
                resolved
            } else {
                file.clone()
            };
            let absolute = root.join(&resolved);

            if link_path.is_some() && resolved == *file {
                errors.push(format!(
                    "{}:{line}: self-link `{target}`; use a section anchor instead",
                    file.display()
                ));
            } else if !absolute.exists() {
                errors.push(format!(
                    "{}:{line}: broken local link `{target}` (resolved to `{}`)",
                    file.display(),
                    resolved.display()
                ));
            } else if absolute.is_file() && known.contains(&resolved) {
                outgoing.insert(resolved.clone());
            }

            if let Some(fragment) = fragment {
                if absolute.is_file()
                    && known.contains(&resolved)
                    && !anchors
                        .get(&resolved)
                        .is_some_and(|headings| headings.contains(&fragment))
                {
                    errors.push(format!(
                        "{}:{line}: missing section `#{fragment}` in `{}`",
                        file.display(),
                        resolved.display()
                    ));
                }
            }
        }

        edges.insert(file.clone(), outgoing);
    }

    let mut reachable = BTreeSet::new();
    let mut queue = VecDeque::from([PathBuf::from("README.md")]);
    while let Some(file) = queue.pop_front() {
        if !reachable.insert(file.clone()) {
            continue;
        }
        if let Some(targets) = edges.get(&file) {
            queue.extend(targets.iter().cloned());
        }
    }

    for file in &files {
        if is_public_doc(file) && !reachable.contains(file) {
            errors.push(format!(
                "{}: orphan document; add a path to it from README.md or the documentation index",
                file.display()
            ));
        }
    }

    errors.sort();
    if errors.is_empty() {
        println!("docs: ok ({} Markdown files)", files.len());
    } else {
        for error in &errors {
            eprintln!("docs: FAIL {error}");
        }
        eprintln!("docs: {} error(s)", errors.len());
        std::process::exit(1);
    }
}

fn repository_root() -> PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap_or_else(|error| fail(&format!("cannot run git: {error}")));
    if !output.status.success() {
        fail("not inside a Git repository");
    }
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

fn markdown_files(root: &Path) -> Vec<PathBuf> {
    let output = Command::new("git")
        .current_dir(root)
        .args([
            "ls-files",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            "*.md",
        ])
        .output()
        .unwrap_or_else(|error| fail(&format!("cannot list Markdown files: {error}")));
    if !output.status.success() {
        fail("git ls-files failed");
    }

    let mut files: Vec<_> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(PathBuf::from)
        .filter(|path| {
            fs::symlink_metadata(root.join(path))
                .map(|metadata| !metadata.file_type().is_symlink())
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files.dedup();
    files
}

fn local_links(source: &str) -> Vec<(usize, String)> {
    let mut links = Vec::new();
    let mut fenced = false;

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }

        let bytes = line.as_bytes();
        let mut cursor = 0;
        let mut inline_code = false;
        while cursor < bytes.len() {
            if bytes[cursor] == b'`' {
                inline_code = !inline_code;
                cursor += 1;
                continue;
            }
            if !inline_code
                && cursor + 1 < bytes.len()
                && bytes[cursor] == b']'
                && bytes[cursor + 1] == b'('
            {
                let start = cursor + 2;
                if let Some(end_offset) = line[start..].find(')') {
                    let raw = line[start..start + end_offset].trim();
                    let target = if let Some(stripped) = raw.strip_prefix('<') {
                        stripped.split_once('>').map_or(raw, |(path, _)| path)
                    } else {
                        raw.split_whitespace().next().unwrap_or("")
                    };
                    links.push((index + 1, target.to_owned()));
                    cursor = start + end_offset + 1;
                    continue;
                }
            }
            cursor += 1;
        }
    }
    links
}

fn local_target(target: &str) -> Option<(Option<PathBuf>, Option<String>)> {
    if target.is_empty() {
        return None;
    }
    let lower = target.to_ascii_lowercase();
    if ["http://", "https://", "mailto:", "tel:", "data:"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
    {
        return None;
    }
    let (path_and_query, fragment) = target
        .split_once('#')
        .map_or((target, None), |(path, anchor)| {
            (path, Some(anchor.to_owned()))
        });
    let path = path_and_query.split('?').next().unwrap_or("");
    Some((
        (!path.is_empty()).then(|| PathBuf::from(path)),
        fragment.filter(|anchor| !anchor.is_empty()),
    ))
}

fn heading_anchors(source: &str) -> BTreeSet<String> {
    let mut anchors = BTreeSet::new();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut fenced = false;

    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
        if !(1..=6).contains(&hashes) || trimmed.as_bytes().get(hashes) != Some(&b' ') {
            continue;
        }

        let heading = trimmed[hashes + 1..].trim().trim_end_matches('#').trim();
        let mut slug = String::new();
        for character in heading.chars() {
            if character.is_alphanumeric() || character == '-' || character == '_' {
                slug.extend(character.to_lowercase());
            } else if character.is_whitespace() {
                slug.push('-');
            }
        }
        let count = counts.entry(slug.clone()).or_default();
        let anchor = if *count == 0 {
            slug.clone()
        } else {
            format!("{slug}-{count}")
        };
        *count += 1;
        anchors.insert(anchor);
    }
    anchors
}

fn normalize(base: &Path, target: &Path) -> Option<PathBuf> {
    if target.is_absolute() {
        return None;
    }
    let joined = base.join(target);
    let mut normalized = PathBuf::new();
    for component in joined.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return None;
                }
            }
            Component::Normal(part) => normalized.push(part),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(normalized)
}

fn is_public_doc(path: &Path) -> bool {
    let text = path.to_string_lossy();
    !text.starts_with(".agents/")
        && !text.starts_with(".github/")
        && path.file_name().is_none_or(|name| name != "AGENTS.md")
}

fn fail(message: &str) -> ! {
    eprintln!("docs: FAIL {message}");
    std::process::exit(1)
}
