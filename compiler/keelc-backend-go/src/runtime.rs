use crate::{BackendError, Emitter};

impl<'a> Emitter<'a> {
    pub(super) fn emit_runtime(&mut self) -> Result<(), BackendError> {
        self.line("type KeelEnum struct {")?;
        self.indent += 1;
        self.line("tag string")?;
        self.line("values []any")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("type keelTask struct {")?;
        self.indent += 1;
        self.line("value any")?;
        self.line("result KeelEnum")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func Some(value any) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"Some\", values: []any{value}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("var None = KeelEnum{tag: \"None\"}")?;
        self.line("")?;
        self.line("var Cancelled = KeelEnum{tag: \"Cancelled\"}")?;
        self.line("")?;
        self.line("func keelPrint(values ...any) {")?;
        self.indent += 1;
        self.line("s := fmt.Sprint(values...)")?;
        self.line("if len(s) > 0 && s[len(s)-1] == ' ' { s = s[:len(s)-1] }")?;
        self.line("fmt.Println(s)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func Ok(value any) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"Ok\", values: []any{value}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func Err(value any) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"Err\", values: []any{value}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.emit_checked_op("checked_div", "KeelEnum", "/", "return None", "Some")?;
        self.emit_checked_op("checked_rem", "KeelEnum", "%", "return None", "Some")?;
        self.emit_checked_op(
            "keelDiv",
            "int64",
            "/",
            r#"panic("K0204: division by zero")"#,
            "",
        )?;
        self.emit_checked_op(
            "keelRem",
            "int64",
            "%",
            r#"panic("K0204: remainder by zero")"#,
            "",
        )?;
        if self.uses_concurrency {
            self.line("type keelWaitGroup = sync.WaitGroup")?;
            self.line("type keelMutex = sync.Mutex")?;
            self.line("")?;
            self.emit_time_runtime()?;
        }
        if self.uses_json {
            self.emit_json_runtime()?;
        }
        if self.uses_http {
            self.emit_http_runtime()?;
        }
        if self.uses_log {
            self.emit_log_runtime()?;
        }
        Ok(())
    }

    fn emit_time_runtime(&mut self) -> Result<(), BackendError> {
        self.line("func keelDuration(value int64, unit time.Duration) time.Duration {")?;
        self.indent += 1;
        self.line("if value < 0 { panic(\"K1501: negative duration\") }")?;
        self.line(
            "if value > int64(^uint64(0)>>1)/int64(unit) { panic(\"K0203: duration overflow\") }",
        )?;
        self.line("return time.Duration(value) * unit")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelCheckCancel(ctx context.Context) KeelEnum {")?;
        self.indent += 1;
        self.line("select {")?;
        self.indent += 1;
        self.line("case <-ctx.Done(): return Err(Cancelled)")?;
        self.line("default: return Ok(struct{}{})")?;
        self.indent -= 1;
        self.line("}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelSleep(ctx context.Context, duration time.Duration) KeelEnum {")?;
        self.indent += 1;
        self.line("timer := time.NewTimer(duration)")?;
        self.line("defer timer.Stop()")?;
        self.line("select {")?;
        self.indent += 1;
        self.line("case <-ctx.Done(): return Err(Cancelled)")?;
        self.line("case <-timer.C: return Ok(struct{}{})")?;
        self.indent -= 1;
        self.line("}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        Ok(())
    }

    fn emit_json_runtime(&mut self) -> Result<(), BackendError> {
        self.line("type keelJSONField struct { name string; value keelJSONValue }")?;
        self.line("type keelJSONValue struct {")?;
        self.indent += 1;
        self.line("kind string")?;
        self.line("text string")?;
        self.line("boolean bool")?;
        self.line("fields []keelJSONField")?;
        self.line("items []keelJSONValue")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelJSONError(tag string, values ...any) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: tag, values: values}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelJSONType(path string, expected string) KeelEnum {")?;
        self.indent += 1;
        self.line("return keelJSONError(\"TypeMismatch\", path, expected)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelJSONRead(dec *json.Decoder, path string) (keelJSONValue, KeelEnum) {")?;
        self.indent += 1;
        self.line("token, err := dec.Token()")?;
        self.line("if err != nil { return keelJSONValue{}, keelJSONError(\"Syntax\", int64(dec.InputOffset())) }")?;
        self.line("switch value := token.(type) {")?;
        self.indent += 1;
        self.line("case nil:")?;
        self.indent += 1;
        self.line("return keelJSONValue{kind: \"null\"}, KeelEnum{}")?;
        self.indent -= 1;
        self.line("case bool:")?;
        self.indent += 1;
        self.line("return keelJSONValue{kind: \"bool\", boolean: value}, KeelEnum{}")?;
        self.indent -= 1;
        self.line("case string:")?;
        self.indent += 1;
        self.line("return keelJSONValue{kind: \"string\", text: value}, KeelEnum{}")?;
        self.indent -= 1;
        self.line("case json.Number:")?;
        self.indent += 1;
        self.line("return keelJSONValue{kind: \"number\", text: value.String()}, KeelEnum{}")?;
        self.indent -= 1;
        self.line("case json.Delim:")?;
        self.indent += 1;
        self.line("switch value {")?;
        self.indent += 1;
        self.line("case '{':")?;
        self.indent += 1;
        self.line("result := keelJSONValue{kind: \"object\"}")?;
        self.line("seen := make(map[string]bool)")?;
        self.line("for dec.More() {")?;
        self.indent += 1;
        self.line("nameToken, nameErr := dec.Token()")?;
        self.line("if nameErr != nil { return keelJSONValue{}, keelJSONError(\"Syntax\", int64(dec.InputOffset())) }")?;
        self.line("name, ok := nameToken.(string)")?;
        self.line("if !ok { return keelJSONValue{}, keelJSONError(\"Syntax\", int64(dec.InputOffset())) }")?;
        self.line("fieldPath := path + \".\" + name")?;
        self.line("if seen[name] { return keelJSONValue{}, keelJSONError(\"DuplicateField\", fieldPath) }")?;
        self.line("seen[name] = true")?;
        self.line("fieldValue, fieldErr := keelJSONRead(dec, fieldPath)")?;
        self.line("if fieldErr.tag != \"\" { return keelJSONValue{}, fieldErr }")?;
        self.line(
            "result.fields = append(result.fields, keelJSONField{name: name, value: fieldValue})",
        )?;
        self.indent -= 1;
        self.line("}")?;
        self.line("if _, endErr := dec.Token(); endErr != nil { return keelJSONValue{}, keelJSONError(\"Syntax\", int64(dec.InputOffset())) }")?;
        self.line("return result, KeelEnum{}")?;
        self.indent -= 1;
        self.line("case '[':")?;
        self.indent += 1;
        self.line("result := keelJSONValue{kind: \"array\"}")?;
        self.line("for index := 0; dec.More(); index++ {")?;
        self.indent += 1;
        self.line("item, itemErr := keelJSONRead(dec, fmt.Sprintf(\"%s[%d]\", path, index))")?;
        self.line("if itemErr.tag != \"\" { return keelJSONValue{}, itemErr }")?;
        self.line("result.items = append(result.items, item)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("if _, endErr := dec.Token(); endErr != nil { return keelJSONValue{}, keelJSONError(\"Syntax\", int64(dec.InputOffset())) }")?;
        self.line("return result, KeelEnum{}")?;
        self.indent -= 1;
        self.line("}")?;
        self.indent -= 1;
        self.line("return keelJSONValue{}, keelJSONError(\"Syntax\", int64(dec.InputOffset()))")?;
        self.indent -= 1;
        self.indent -= 1;
        self.line("}")?;
        self.line("return keelJSONValue{}, keelJSONError(\"Syntax\", int64(dec.InputOffset()))")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelJSONFirstNonSpace(input string, start int) int64 {")?;
        self.indent += 1;
        self.line("for start < len(input) {")?;
        self.indent += 1;
        self.line("switch input[start] { case ' ', '\\n', '\\r', '\\t': start++; default: return int64(start) }")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("return int64(start)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelJSONParseRaw(input string) KeelEnum {")?;
        self.indent += 1;
        self.line("dec := json.NewDecoder(strings.NewReader(input))")?;
        self.line("dec.UseNumber()")?;
        self.line("value, parseErr := keelJSONRead(dec, \"$\")")?;
        self.line("if parseErr.tag != \"\" { return Err(parseErr) }")?;
        self.line("end := int(dec.InputOffset())")?;
        self.line("_, trailingErr := dec.Token()")?;
        self.line("if trailingErr != io.EOF {")?;
        self.indent += 1;
        self.line("if trailingErr != nil { return Err(keelJSONError(\"Syntax\", int64(dec.InputOffset()))) }")?;
        self.line("return Err(keelJSONError(\"Syntax\", keelJSONFirstNonSpace(input, end)))")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("return Ok(value)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelJSONSchemaDrift(path string) { _ = path }")?;
        self.line("func keelJSONNonFinite(value float64) bool { return math.IsInf(value, 0) || math.IsNaN(value) }")?;
        self.line("")?;
        Ok(())
    }

    fn emit_http_runtime(&mut self) -> Result<(), BackendError> {
        self.line("type keelHTTPResponse struct {")?;
        self.indent += 1;
        self.line("status int64")?;
        self.line("body string")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("type keelHTTPRequest struct {")?;
        self.indent += 1;
        self.line("body string")?;
        self.line("method string")?;
        self.line("path string")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func BindFailed(message string) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"BindFailed\", values: []any{message}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        if self.uses_http_serve {
            self.line("func keelHTTPServe(port int64, handler func(keelHTTPRequest) keelHTTPResponse) KeelEnum {")?;
            self.indent += 1;
            self.line("mux := http.NewServeMux()")?;
            self.line("mux.HandleFunc(\"/\", func(w http.ResponseWriter, r *http.Request) {")?;
            self.indent += 1;
            self.line("body, _ := io.ReadAll(r.Body)")?;
            self.line(
                "req := keelHTTPRequest{body: string(body), method: r.Method, path: r.URL.Path}",
            )?;
            self.line("resp := handler(req)")?;
            self.line("w.WriteHeader(int(resp.status))")?;
            self.line("_, _ = w.Write([]byte(resp.body))")?;
            self.indent -= 1;
            self.line("})")?;
            self.line("err := http.ListenAndServe(fmt.Sprintf(\":%d\", port), mux)")?;
            self.line("if err != nil {")?;
            self.indent += 1;
            self.line("return Err(BindFailed(err.Error()))")?;
            self.indent -= 1;
            self.line("}")?;
            self.line("return Ok(struct{}{})")?;
            self.indent -= 1;
            self.line("}")?;
            self.line("")?;
        }
        Ok(())
    }

    fn emit_log_runtime(&mut self) -> Result<(), BackendError> {
        self.line("func keelLogInfo(msg string) {")?;
        self.indent += 1;
        self.line("fmt.Println(\"[info]\", msg)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelLogWarn(msg string) {")?;
        self.indent += 1;
        self.line("fmt.Println(\"[warn]\", msg)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelLogError(msg string) {")?;
        self.indent += 1;
        self.line("fmt.Println(\"[error]\", msg)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        Ok(())
    }
}
