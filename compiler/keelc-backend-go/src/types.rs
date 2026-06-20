use keelc_types::TypeInfo;

pub fn go_type(ty: &TypeInfo, struct_names: &[String], interface_names: &[String]) -> String {
    match ty {
        TypeInfo::Named(name) if name == "time.Duration" => "time.Duration".to_string(),
        TypeInfo::Named(name) if struct_names.iter().any(|n| n == name) => name.clone(),
        TypeInfo::Named(name) if interface_names.iter().any(|n| n == name) => name.clone(),
        TypeInfo::Named(name) if name == "http.Response" => "keelHTTPResponse".to_string(),
        TypeInfo::Named(name) if name == "http.Request" => "keelHTTPRequest".to_string(),
        TypeInfo::Named(name) if name == "sql.Pool" => "keelSQLPool".to_string(),
        TypeInfo::Named(name) if name == "sql.QueryResult" => "keelSQLQueryResult".to_string(),
        TypeInfo::Named(name) if name == "sql.Row" => "keelSQLRow".to_string(),
        TypeInfo::Generic { name, .. } if name == "sql.RowMapper" => "keelSQLRowMapper".to_string(),
        TypeInfo::Named(name) if name == "Secret" => "keelConfigSecret".to_string(),
        TypeInfo::Named(name) if name == "Uuid" => "string".to_string(),
        TypeInfo::Named(name) if name == "Timestamp" => "keelTimestamp".to_string(),
        TypeInfo::Named(name) if name == "Email" => "string".to_string(),
        TypeInfo::Int => "int64".to_string(),
        TypeInfo::Float => "float64".to_string(),
        TypeInfo::Bool => "bool".to_string(),
        TypeInfo::String => "string".to_string(),
        TypeInfo::Char => "rune".to_string(),
        TypeInfo::Unit => String::new(),
        TypeInfo::Named(_) | TypeInfo::Generic { .. } | TypeInfo::Union(_) => {
            "KeelEnum".to_string()
        }
        TypeInfo::Interface(name) | TypeInfo::TypeParam { bound: name, .. } => name.clone(),
        TypeInfo::Unknown => "any".to_string(),
    }
}

pub fn zero_value(ty: &TypeInfo) -> &'static str {
    match ty {
        TypeInfo::Named(name) if name == "time.Duration" => "0",
        TypeInfo::Named(name) if name == "http.Response" || name == "http.Request" => {
            "keelHTTPResponse{}"
        }
        TypeInfo::Named(name) if name == "sql.Pool" => "keelSQLPool{}",
        TypeInfo::Named(name) if name == "sql.QueryResult" => "keelSQLQueryResult{}",
        TypeInfo::Named(name) if name == "sql.Row" => "keelSQLRow{}",
        TypeInfo::Generic { name, .. } if name == "sql.RowMapper" => "keelSQLRowMapper{}",
        TypeInfo::Named(name) if name == "Secret" => "keelConfigSecret{}",
        TypeInfo::Named(name) if name == "Uuid" => "\"\"",
        TypeInfo::Named(name) if name == "Timestamp" => "keelTimestamp{}",
        TypeInfo::Named(name) if name == "Email" => "\"\"",
        TypeInfo::Int | TypeInfo::Float | TypeInfo::Char => "0",
        TypeInfo::Bool => "false",
        TypeInfo::String => "\"\"",
        TypeInfo::Unit => "",
        TypeInfo::Named(_) | TypeInfo::Generic { .. } | TypeInfo::Union(_) => "KeelEnum{}",
        TypeInfo::Interface(_) | TypeInfo::TypeParam { .. } => "nil",
        TypeInfo::Unknown => "nil",
    }
}

pub fn go_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}

pub fn go_binary_op(op: keelc_kir::BinaryOp) -> &'static str {
    match op {
        keelc_kir::BinaryOp::Add => "+",
        keelc_kir::BinaryOp::Subtract => "-",
        keelc_kir::BinaryOp::Multiply => "*",
        keelc_kir::BinaryOp::Divide => "/",
        keelc_kir::BinaryOp::Remainder => "%",
        keelc_kir::BinaryOp::Equal => "==",
        keelc_kir::BinaryOp::NotEqual => "!=",
        keelc_kir::BinaryOp::Less => "<",
        keelc_kir::BinaryOp::LessEqual => "<=",
        keelc_kir::BinaryOp::Greater => ">",
        keelc_kir::BinaryOp::GreaterEqual => ">=",
        keelc_kir::BinaryOp::And => "&&",
        keelc_kir::BinaryOp::Or => "||",
    }
}

pub fn primitive_underlying(type_name: &str) -> Option<&'static str> {
    match type_name {
        "Int" => Some("int64"),
        "Float" => Some("float64"),
        "Bool" => Some("bool"),
        "String" => Some("string"),
        "Char" => Some("rune"),
        _ => None,
    }
}

pub fn primitive_box_name(ty: &TypeInfo) -> Option<&'static str> {
    match ty {
        TypeInfo::Int => Some("keelBox_Int"),
        TypeInfo::Float => Some("keelBox_Float"),
        TypeInfo::Bool => Some("keelBox_Bool"),
        TypeInfo::String => Some("keelBox_String"),
        TypeInfo::Char => Some("keelBox_Char"),
        _ => None,
    }
}

pub fn json_type_name(ty: &TypeInfo) -> String {
    match ty {
        TypeInfo::Int => "Int".to_string(),
        TypeInfo::Float => "Float".to_string(),
        TypeInfo::Bool => "Bool".to_string(),
        TypeInfo::String => "String".to_string(),
        TypeInfo::Char => "Char".to_string(),
        TypeInfo::Unit => "Unit".to_string(),
        TypeInfo::Named(name) | TypeInfo::Interface(name) => name.replace('.', "_"),
        TypeInfo::TypeParam { name, .. } => name.clone(),
        TypeInfo::Generic { name, args } => format!(
            "{}_{}",
            name,
            args.iter()
                .map(json_type_name)
                .collect::<Vec<_>>()
                .join("_")
        ),
        TypeInfo::Union(members) => format!(
            "Union_{}",
            members
                .iter()
                .map(json_type_name)
                .collect::<Vec<_>>()
                .join("_")
        ),
        TypeInfo::Unknown => "Unknown".to_string(),
    }
}
