# Keel Brand Style Guide

Keel is a typed, compiled, garbage-collected programming language for durable backend services. The brand should feel engineered, stable, operationally serious, and readable after years of team churn.

## 1. Brand Positioning

**Core idea:**  
Keel is the backbone of backend systems: structured, safe, long-lived, and deployable.

**Brand promise:**  
Write backend software that remains readable, reviewable, and operationally dependable five years later.

**Tone:**  
- Precise
- Calm
- Direct
- Engineering-first
- Anti-hype
- Long-termist

**Avoid:**  
- Startup-glossy exaggeration
- Cute mascot energy
- Magical language claims
- Overly academic positioning
- “Move fast and break things” rhetoric

## 2. Logo Concept

The Keel mark combines two ideas:

1. **K monogram** — a direct reference to the language name and to code structure.
2. **Ship keel / hull** — a metaphor for stability, direction, and long service life.

The logo should communicate that Keel is a practical systems language for teams who care about maintainability, safety, and production operations.

## 3. Logo Files

Recommended file names:

- `keel-logo.svg` — primary standalone icon
- `keel-logo-horizontal.svg` — icon + wordmark lockup

Use SVG as the primary format for documentation, websites, GitHub READMEs, package registries, and conference materials. Export PNGs only when a platform does not support SVG.

## 4. Primary Logo

The primary logo is the standalone geometric K + keel mark.

Use it for:

- Language icon
- CLI identity
- Documentation favicon
- GitHub organization avatar
- Package registry icon
- Release badges
- Social avatars

Minimum recommended size:

- Digital UI: **24 px**
- Documentation header: **40 px**
- Social/avatar use: **256 px**

At sizes below 24 px, simplify the hull ribs or use a flat two-color version.

## 5. Horizontal Lockup

The horizontal lockup pairs the logo with the Keel wordmark.

Use it for:

- Website headers
- Documentation landing pages
- Conference slides
- README hero sections
- Release announcements

Do not recreate the wordmark manually unless the same spacing, font, and proportions are preserved.

## 6. Clear Space

Keep clear space around the logo equal to at least **25% of the icon width**.

For example:

- If the icon is 256 px wide, keep at least 64 px of empty space around it.
- Do not place text, badges, icons, or borders inside this space.

## 7. Color Palette

| Token | Name | Hex | Use |
|---|---:|---:|---|
| `--keel-navy` | Deep Keel Navy | `#0F172A` | Primary logo, headings, high-emphasis UI |
| `--keel-teal` | Harbor Teal | `#0F766E` | Primary accent, hull, links, active states |
| `--keel-seaglass` | Sea Glass | `#99F6E4` | Secondary accent, highlights, subtle fills |
| `--keel-slate` | Steel Slate | `#475569` | Body text, captions, secondary UI |
| `--keel-coral` | Signal Coral | `#F97360` | Warnings, migration notices, breaking-change callouts |
| `--keel-mist` | Mist Background | `#F8FAFC` | Page background, cards, documentation surfaces |

### CSS Tokens

```css
:root {
  --keel-navy: #0F172A;
  --keel-teal: #0F766E;
  --keel-seaglass: #99F6E4;
  --keel-slate: #475569;
  --keel-coral: #F97360;
  --keel-mist: #F8FAFC;
}
```

## 8. Color Usage

### Recommended Ratios

- **70% Mist Background / white space**
- **20% Deep Keel Navy and Steel Slate**
- **8% Harbor Teal and Sea Glass**
- **2% Signal Coral**

Signal Coral should be rare. Use it for important operational or migration-related emphasis, not decoration.

### Accessibility Notes

Use Deep Keel Navy for text on Mist Background.  
Use Harbor Teal for accents, not long paragraphs.  
Avoid Sea Glass as text on light backgrounds because contrast will usually be too weak.

## 9. Typography

**Primary typeface:** JetBrains Mono

Use JetBrains Mono across:

- Logo wordmark
- Documentation headings
- Code examples
- CLI examples
- Navigation labels
- Technical diagrams

Recommended fallback stack:

```css
font-family:
  "JetBrains Mono",
  "SFMono-Regular",
  Consolas,
  "Liberation Mono",
  Menlo,
  monospace;
```

### Type Scale

| Role | Size | Weight | Color |
|---|---:|---:|---|
| Hero title | 56–72 px | 700 | Deep Keel Navy |
| Page H1 | 40–48 px | 700 | Deep Keel Navy |
| H2 | 28–32 px | 650 | Deep Keel Navy |
| H3 | 20–24 px | 600 | Deep Keel Navy |
| Body | 16–18 px | 400 | Steel Slate |
| Caption | 13–14 px | 400 | Steel Slate |
| Code | 14–16 px | 400/500 | Deep Keel Navy |

## 10. Wordmark

The wordmark should read:

```text
Keel
```

Preferred styling:

- JetBrains Mono
- Bold or SemiBold
- Deep Keel Navy
- Slightly tightened tracking for large display use
- No gradients
- No outlines
- No drop shadows

Do not write it as:

- `KEEL`
- `keel`
- `KeelLang`
- `Keel Language`

Use **Keel** unless referring to a package, binary, or command.

## 11. Tagline

Primary tagline:

> A typed, compiled language for durable backend systems.

Alternative short forms:

- Typed. Compiled. Durable.
- Backend systems that survive team churn.
- Structured backend software for the long run.
- Safe procedural code over plain data.

Avoid:

- “The future of backend development”
- “Rust without the borrow checker”
- “Go but better”
- “Enterprise-grade language revolution”

## 12. Brand Traits

Use these words consistently:

- Typed
- Stable
- Readable
- Structured
- Operational
- Long-lived

Expanded meaning:

| Trait | Meaning |
|---|---|
| Typed | Static guarantees, explicit data, predictable interfaces |
| Stable | Boring by design, reliable under production pressure |
| Readable | Code review and maintenance matter more than cleverness |
| Structured | Clear control flow, structured concurrency, explicit capabilities |
| Operational | Built for health checks, cgroups, telemetry, and deployment |
| Long-lived | Designed for five-year maintainability, editions, and migration tooling |

## 13. Icon Usage Rules

Do:

- Use the SVG source whenever possible.
- Keep the icon flat and geometric.
- Preserve the navy / teal relationship.
- Use the standalone mark when space is tight.
- Use the horizontal lockup when introducing the project.

Do not:

- Rotate the icon.
- Add gradients or 3D effects.
- Add mascot eyes, waves, or decorative ship details.
- Place the icon on busy photography.
- Change the hull into a literal boat illustration.
- Use Signal Coral as a primary logo color.

## 14. Backgrounds

Preferred backgrounds:

- Mist Background `#F8FAFC`
- White `#FFFFFF`
- Deep Keel Navy `#0F172A` for dark-mode hero sections

For dark backgrounds, use:

- Sea Glass for the hull accents
- White or Mist Background for wordmark text
- Avoid low-contrast Steel Slate text

## 15. Documentation Style

Documentation should look like the language behaves:

- Minimal configuration
- Predictable structure
- Stable layout
- Clear hierarchy
- Strong code readability
- No novelty UI unless it improves comprehension

Recommended doc components:

- Navy headings
- Slate body text
- Teal links
- Sea Glass callout backgrounds
- Coral warning strips only for breaking changes or security-sensitive notes

## 16. Example CLI Presentation

```text
keel build
keel run
keel test
keel fmt
keel lint
keel audit
keel gen
keel fix
```

CLI examples should be direct, copyable, and visually restrained.

## 17. Example README Header

```md
<p align="center">
  <img src="./assets/keel-logo.svg" width="120" alt="Keel logo">
</p>

<h1 align="center">Keel</h1>

<p align="center">
  A typed, compiled language for durable backend systems.
</p>
```

## 18. Design Principle

Keel should not look experimental for its own sake. It should look like a tool a backend team can trust in production, document clearly, and still understand years later.
