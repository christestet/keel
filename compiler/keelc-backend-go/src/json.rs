use crate::analysis::StructInfo;
use crate::types::json_type_name;
use crate::{BackendError, Emitter};
use keelc_kir::Item;
use keelc_types::TypeInfo;

impl<'a> Emitter<'a> {
    pub(super) fn emit_json_codecs(&mut self) -> Result<(), BackendError> {
        if !self.uses_json {
            return Ok(());
        }
        let mut index = 0;
        while index < self.json_types.len() {
            let ty = self.json_types[index].clone();
            self.register_json_children(&ty);
            self.emit_json_decoder(&ty)?;
            self.line("")?;
            self.emit_json_encoder(&ty)?;
            self.line("")?;
            index += 1;
        }
        Ok(())
    }

    fn register_json_children(&mut self, ty: &TypeInfo) {
        match ty {
            TypeInfo::Generic { args, .. } => {
                for arg in args {
                    if arg != &TypeInfo::String
                        || !matches!(ty, TypeInfo::Generic { name, .. } if name == "Map")
                    {
                        self.register_json_type(arg);
                    }
                }
            }
            TypeInfo::Named(name) => {
                let struct_fields = self
                    .structs
                    .iter()
                    .find(|info| info.name == *name)
                    .map(|info| info.fields.clone())
                    .unwrap_or_default();
                for field in struct_fields {
                    self.register_json_type(&field.ty);
                }
                let enum_fields = self
                    .module
                    .items
                    .iter()
                    .find_map(|item| match item {
                        Item::Enum(decl) if decl.name == *name => Some(
                            decl.variants
                                .iter()
                                .flat_map(|variant| variant.fields.clone())
                                .collect::<Vec<_>>(),
                        ),
                        _ => None,
                    })
                    .unwrap_or_default();
                for field in enum_fields {
                    self.register_json_type(&field.ty);
                }
            }
            _ => {}
        }
    }

    fn emit_json_decoder(&mut self, ty: &TypeInfo) -> Result<(), BackendError> {
        let suffix = json_type_name(ty);
        self.line_fmt(format_args!(
            "func keelJSONParse_{suffix}(input string, tolerant bool) KeelEnum {{"
        ))?;
        self.indent += 1;
        self.line("raw := keelJSONParseRaw(input)")?;
        self.line("if raw.tag == \"Err\" { return raw }")?;
        self.line_fmt(format_args!(
            "return keelJSONDecode_{suffix}(raw.values[0].(keelJSONValue), \"$\", tolerant)"
        ))?;
        self.indent -= 1;
        self.line("}")?;
        self.line_fmt(format_args!(
            "func keelJSONDecode_{suffix}(value keelJSONValue, path string, tolerant bool) KeelEnum {{"
        ))?;
        self.indent += 1;
        match ty {
            TypeInfo::Int => {
                self.line("if value.kind != \"number\" || strings.ContainsAny(value.text, \".eE\") { return Err(keelJSONType(path, \"Int\")) }")?;
                self.line("decoded, err := strconv.ParseInt(value.text, 10, 64)")?;
                self.line("if err != nil { return Err(keelJSONError(\"OutOfRange\", path)) }")?;
                self.line("return Ok(decoded)")?;
            }
            TypeInfo::Float => {
                self.line(
                    "if value.kind != \"number\" { return Err(keelJSONType(path, \"Float\")) }",
                )?;
                self.line("decoded, err := strconv.ParseFloat(value.text, 64)")?;
                self.line("if err != nil || keelJSONNonFinite(decoded) { return Err(keelJSONError(\"OutOfRange\", path)) }")?;
                self.line("return Ok(decoded)")?;
            }
            TypeInfo::Bool => {
                self.line(
                    "if value.kind != \"bool\" { return Err(keelJSONType(path, \"Bool\")) }",
                )?;
                self.line("return Ok(value.boolean)")?;
            }
            TypeInfo::String => {
                self.line(
                    "if value.kind != \"string\" { return Err(keelJSONType(path, \"String\")) }",
                )?;
                self.line("return Ok(value.text)")?;
            }
            TypeInfo::Char => {
                self.line(
                    "if value.kind != \"string\" { return Err(keelJSONType(path, \"Char\")) }",
                )?;
                self.line("runes := []rune(value.text)")?;
                self.line("if len(runes) != 1 { return Err(keelJSONType(path, \"Char\")) }")?;
                self.line("return Ok(runes[0])")?;
            }
            TypeInfo::Named(name) if name == "Uuid" => {
                self.line("if value.kind != \"string\" || !keelUUIDValid(value.text) { return Err(keelJSONType(path, \"Uuid\")) }")?;
                self.line("return Ok(value.text)")?;
            }
            TypeInfo::Named(name) if name == "Timestamp" => {
                self.line(
                    "if value.kind != \"string\" { return Err(keelJSONType(path, \"Timestamp\")) }",
                )?;
                self.line("decoded, ok := keelTimestampParse(value.text)")?;
                self.line("if !ok { return Err(keelJSONType(path, \"Timestamp\")) }")?;
                self.line("return Ok(decoded)")?;
            }
            TypeInfo::Named(name) if name == "Email" => {
                self.line(
                    "if value.kind != \"string\" { return Err(keelJSONType(path, \"Email\")) }",
                )?;
                self.line("decoded, ok := keelEmailParse(value.text)")?;
                self.line("if !ok { return Err(keelJSONType(path, \"Email\")) }")?;
                self.line("return Ok(decoded)")?;
            }
            TypeInfo::Generic { name, args } if name == "Option" && args.len() == 1 => {
                let inner = &args[0];
                let inner_suffix = json_type_name(inner);
                let inner_go = self.go_type(inner);
                self.line("if value.kind == \"null\" { return Ok(None) }")?;
                self.line_fmt(format_args!(
                    "decoded := keelJSONDecode_{inner_suffix}(value, path, tolerant)"
                ))?;
                self.line("if decoded.tag == \"Err\" { return decoded }")?;
                self.line_fmt(format_args!(
                    "return Ok(Some(decoded.values[0].({inner_go})))"
                ))?;
            }
            TypeInfo::Generic { name, args } if name == "List" && args.len() == 1 => {
                let inner = &args[0];
                let inner_suffix = json_type_name(inner);
                self.line("if value.kind != \"array\" { return Err(keelJSONType(path, \"List\")) }")?;
                self.line("out := []any{}")?;
                self.line("for _, item := range value.items {")?;
                self.indent += 1;
                self.line_fmt(format_args!(
                    "decoded := keelJSONDecode_{inner_suffix}(item, path + \"[]\", tolerant)"
                ))?;
                self.line("if decoded.tag == \"Err\" { return decoded }")?;
                self.line("out = append(out, decoded.values[0])")?;
                self.indent -= 1;
                self.line("}")?;
                self.line("return Ok(out)")?;
            }
            TypeInfo::Named(name) => {
                if let Some(info) = self.structs.iter().find(|info| info.name == *name).cloned() {
                    self.emit_json_struct_decoder(&info)?;
                } else if let Some(decl) = self.module.items.iter().find_map(|item| match item {
                    Item::Enum(decl) if decl.name == *name => Some(decl.clone()),
                    _ => None,
                }) {
                    self.emit_json_enum_decoder(&decl)?;
                } else {
                    return Err(BackendError::unsupported(format!("JSON type `{name}`")));
                }
            }
            _ => return Err(BackendError::unsupported(format!("JSON type `{ty}`"))),
        }
        self.indent -= 1;
        self.line("}")
    }

    fn emit_json_struct_decoder(&mut self, info: &StructInfo) -> Result<(), BackendError> {
        self.line_fmt(format_args!(
            "if value.kind != \"object\" {{ return Err(keelJSONType(path, {:?})) }}",
            info.name
        ))?;
        self.line_fmt(format_args!("var decoded {}", info.name))?;
        for field in &info.fields {
            self.line_fmt(format_args!("has_{} := false", field.name))?;
        }
        self.line("for _, field := range value.fields {")?;
        self.indent += 1;
        self.line("fieldPath := path + \".\" + field.name")?;
        self.line("switch field.name {")?;
        self.indent += 1;
        for field in &info.fields {
            let suffix = json_type_name(&field.ty);
            let go_ty = self.go_type(&field.ty);
            self.line_fmt(format_args!("case {:?}:", field.name))?;
            self.indent += 1;
            self.line_fmt(format_args!("has_{} = true", field.name))?;
            self.line_fmt(format_args!(
                "fieldResult := keelJSONDecode_{suffix}(field.value, fieldPath, tolerant)"
            ))?;
            self.line("if fieldResult.tag == \"Err\" { return fieldResult }")?;
            self.line_fmt(format_args!(
                "decoded.{} = fieldResult.values[0].({go_ty})",
                field.name
            ))?;
            self.indent -= 1;
        }
        self.line("default:")?;
        self.indent += 1;
        self.line("if !tolerant { return Err(keelJSONError(\"UnknownField\", fieldPath)) }")?;
        self.line("keelJSONSchemaDrift(fieldPath)")?;
        self.indent -= 1;
        self.indent -= 1;
        self.line("}")?;
        self.indent -= 1;
        self.line("}")?;
        for field in &info.fields {
            if field.ty.option_inner().is_some() {
                self.line_fmt(format_args!(
                    "if !has_{} {{ decoded.{} = None }}",
                    field.name, field.name
                ))?;
            } else {
                self.line_fmt(format_args!(
                    "if !has_{} {{ return Err(keelJSONError(\"MissingField\", path + {:?})) }}",
                    field.name,
                    format!(".{}", field.name)
                ))?;
            }
        }
        self.line("return Ok(decoded)")
    }

    fn emit_json_enum_decoder(&mut self, decl: &keelc_kir::EnumDecl) -> Result<(), BackendError> {
        self.line_fmt(format_args!(
            "if value.kind != \"object\" {{ return Err(keelJSONType(path, {:?})) }}",
            decl.name
        ))?;
        self.line("var variant string")?;
        self.line("var fields keelJSONValue")?;
        self.line("hasVariant := false")?;
        self.line("hasFields := false")?;
        self.line("for _, field := range value.fields {")?;
        self.indent += 1;
        self.line("fieldPath := path + \".\" + field.name")?;
        self.line("switch field.name {")?;
        self.indent += 1;
        self.line("case \"variant\":")?;
        self.indent += 1;
        self.line(
            "if field.value.kind != \"string\" { return Err(keelJSONType(fieldPath, \"String\")) }",
        )?;
        self.line("variant = field.value.text; hasVariant = true")?;
        self.indent -= 1;
        self.line("case \"fields\": fields = field.value; hasFields = true")?;
        self.line("default:")?;
        self.indent += 1;
        self.line("if !tolerant { return Err(keelJSONError(\"UnknownField\", fieldPath)) }; keelJSONSchemaDrift(fieldPath)")?;
        self.indent -= 1;
        self.indent -= 1;
        self.line("}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line(
            "if !hasVariant { return Err(keelJSONError(\"MissingField\", path + \".variant\")) }",
        )?;
        self.line(
            "if !hasFields { return Err(keelJSONError(\"MissingField\", path + \".fields\")) }",
        )?;
        self.line("if fields.kind != \"object\" { return Err(keelJSONType(path + \".fields\", \"object\")) }")?;
        self.line("switch variant {")?;
        self.indent += 1;
        for variant in &decl.variants {
            self.line_fmt(format_args!("case {:?}:", variant.name))?;
            self.indent += 1;
            for field in &variant.fields {
                self.line_fmt(format_args!("var raw_{} keelJSONValue", field.name))?;
                self.line_fmt(format_args!("has_{} := false", field.name))?;
            }
            self.line("for _, field := range fields.fields {")?;
            self.indent += 1;
            self.line("fieldPath := path + \".fields.\" + field.name")?;
            self.line("switch field.name {")?;
            self.indent += 1;
            for field in &variant.fields {
                self.line_fmt(format_args!(
                    "case {:?}: raw_{} = field.value; has_{} = true",
                    field.name, field.name, field.name
                ))?;
            }
            self.line("default: if !tolerant { return Err(keelJSONError(\"UnknownField\", fieldPath)) }; keelJSONSchemaDrift(fieldPath)")?;
            self.indent -= 1;
            self.line("}")?;
            self.indent -= 1;
            self.line("}")?;
            let mut values = Vec::new();
            for field in &variant.fields {
                let suffix = json_type_name(&field.ty);
                let go_ty = self.go_type(&field.ty);
                if field.ty.option_inner().is_some() {
                    self.line_fmt(format_args!(
                        "if !has_{} {{ raw_{} = keelJSONValue{{kind: \"null\"}} }}",
                        field.name, field.name
                    ))?;
                } else {
                    self.line_fmt(format_args!(
                        "if !has_{} {{ return Err(keelJSONError(\"MissingField\", path + {:?})) }}",
                        field.name,
                        format!(".fields.{}", field.name)
                    ))?;
                }
                self.line_fmt(format_args!(
                    "decoded_{} := keelJSONDecode_{suffix}(raw_{}, path + {:?}, tolerant)",
                    field.name,
                    field.name,
                    format!(".fields.{}", field.name)
                ))?;
                self.line_fmt(format_args!(
                    "if decoded_{}.tag == \"Err\" {{ return decoded_{} }}",
                    field.name, field.name
                ))?;
                values.push(format!("decoded_{}.values[0].({go_ty})", field.name));
            }
            if values.is_empty() {
                self.line_fmt(format_args!(
                    "return Ok(KeelEnum{{tag: {:?}}})",
                    variant.name
                ))?;
            } else {
                self.line_fmt(format_args!(
                    "return Ok(KeelEnum{{tag: {:?}, values: []any{{{}}}}})",
                    variant.name,
                    values.join(", ")
                ))?;
            }
            self.indent -= 1;
        }
        self.line_fmt(format_args!(
            "default: return Err(keelJSONType(path + \".variant\", {:?}))",
            decl.name
        ))?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_json_encoder(&mut self, ty: &TypeInfo) -> Result<(), BackendError> {
        let suffix = json_type_name(ty);
        let go_ty = self.go_type(ty);
        self.line_fmt(format_args!(
            "func keelJSONEncode_{suffix}(value {go_ty}, path string) KeelEnum {{"
        ))?;
        self.indent += 1;
        match ty {
            TypeInfo::Int => self.line("return Ok(strconv.FormatInt(value, 10))")?,
            TypeInfo::Float => {
                // JSON has no token for NaN/±Inf; serialise as null (KDR-0040).
                self.line("if keelJSONNonFinite(value) { return Ok(\"null\") }")?;
                self.line("return Ok(strconv.FormatFloat(value, 'g', -1, 64))")?;
            }
            TypeInfo::Bool => self.line("return Ok(strconv.FormatBool(value))")?,
            TypeInfo::String => self.line("return Ok(strconv.Quote(value))")?,
            TypeInfo::Char => self.line("return Ok(strconv.Quote(string(value)))")?,
            TypeInfo::Named(name) if name == "Uuid" => {
                self.line("return Ok(strconv.Quote(value))")?;
            }
            TypeInfo::Named(name) if name == "Timestamp" => {
                self.line("return Ok(strconv.Quote(keelTimestampFormat(value)))")?;
            }
            TypeInfo::Named(name) if name == "Email" => {
                self.line("return Ok(strconv.Quote(value))")?;
            }
            TypeInfo::Generic { name, args } if name == "Option" && args.len() == 1 => {
                let inner = &args[0];
                let inner_suffix = json_type_name(inner);
                let inner_go = self.go_type(inner);
                self.line("if value.tag == \"None\" { return Ok(\"null\") }")?;
                self.line_fmt(format_args!(
                    "return keelJSONEncode_{inner_suffix}(value.values[0].({inner_go}), path)"
                ))?;
            }
            TypeInfo::Generic { name, args } if name == "List" && args.len() == 1 => {
                let inner = &args[0];
                let inner_suffix = json_type_name(inner);
                let inner_go = self.go_type(inner);
                self.line("var out strings.Builder")?;
                self.line("out.WriteByte('[')")?;
                self.line("for i, elem := range value {")?;
                self.indent += 1;
                self.line("if i > 0 { out.WriteByte(',') }")?;
                self.line_fmt(format_args!(
                    "encoded := keelJSONEncode_{inner_suffix}(elem.({inner_go}), path + \"[]\")"
                ))?;
                self.line("if encoded.tag == \"Err\" { return encoded }")?;
                self.line("out.WriteString(encoded.values[0].(string))")?;
                self.indent -= 1;
                self.line("}")?;
                self.line("out.WriteByte(']')")?;
                self.line("return Ok(out.String())")?;
            }
            TypeInfo::Named(name) => {
                if let Some(info) = self.structs.iter().find(|info| info.name == *name).cloned() {
                    self.emit_json_struct_encoder(&info)?;
                } else if let Some(decl) = self.module.items.iter().find_map(|item| match item {
                    Item::Enum(decl) if decl.name == *name => Some(decl.clone()),
                    _ => None,
                }) {
                    self.emit_json_enum_encoder(&decl)?;
                } else {
                    return Err(BackendError::unsupported(format!("JSON type `{name}`")));
                }
            }
            _ => return Err(BackendError::unsupported(format!("JSON type `{ty}`"))),
        }
        self.indent -= 1;
        self.line("}")
    }

    fn emit_json_struct_encoder(&mut self, info: &StructInfo) -> Result<(), BackendError> {
        self.line("var out strings.Builder")?;
        self.line("out.WriteByte('{')")?;
        for (index, field) in info.fields.iter().enumerate() {
            let suffix = json_type_name(&field.ty);
            if index > 0 {
                self.line("out.WriteByte(',')")?;
            }
            self.line_fmt(format_args!(
                "out.WriteString({:?})",
                format!("\"{}\":", field.name)
            ))?;
            self.line_fmt(format_args!(
                "encoded_{} := keelJSONEncode_{suffix}(value.{}, path + {:?})",
                field.name,
                field.name,
                format!(".{}", field.name)
            ))?;
            self.line_fmt(format_args!(
                "if encoded_{}.tag == \"Err\" {{ return encoded_{} }}",
                field.name, field.name
            ))?;
            self.line_fmt(format_args!(
                "out.WriteString(encoded_{}.values[0].(string))",
                field.name
            ))?;
        }
        self.line("out.WriteByte('}')")?;
        self.line("return Ok(out.String())")
    }

    fn emit_json_enum_encoder(&mut self, decl: &keelc_kir::EnumDecl) -> Result<(), BackendError> {
        self.line("var out strings.Builder")?;
        self.line("out.WriteString(\"{\\\"variant\\\":\")")?;
        self.line("out.WriteString(strconv.Quote(value.tag))")?;
        self.line("out.WriteString(\",\\\"fields\\\":{\")")?;
        self.line("switch value.tag {")?;
        self.indent += 1;
        for variant in &decl.variants {
            self.line_fmt(format_args!("case {:?}:", variant.name))?;
            self.indent += 1;
            for (index, field) in variant.fields.iter().enumerate() {
                let suffix = json_type_name(&field.ty);
                let go_ty = self.go_type(&field.ty);
                if index > 0 {
                    self.line("out.WriteByte(',')")?;
                }
                self.line_fmt(format_args!(
                    "out.WriteString({:?})",
                    format!("\"{}\":", field.name)
                ))?;
                self.line_fmt(format_args!(
                    "encoded_{} := keelJSONEncode_{suffix}(value.values[{}].({go_ty}), path + {:?})",
                    field.name,
                    index,
                    format!(".fields.{}", field.name)
                ))?;
                self.line_fmt(format_args!(
                    "if encoded_{}.tag == \"Err\" {{ return encoded_{} }}",
                    field.name, field.name
                ))?;
                self.line_fmt(format_args!(
                    "out.WriteString(encoded_{}.values[0].(string))",
                    field.name
                ))?;
            }
            self.indent -= 1;
        }
        self.line_fmt(format_args!(
            "default: return Err(keelJSONType(path, {:?}))",
            decl.name
        ))?;
        self.indent -= 1;
        self.line("}")?;
        self.line("out.WriteString(\"}}\")")?;
        self.line("return Ok(out.String())")
    }
}
