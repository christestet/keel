use crate::{BackendError, Emitter};

/// `std.sql` runtime (KDR-0029). Handles flow through Keel as `any` (catch/`?`
/// results), so every wrapper takes `any` and asserts internally. Backed by the
/// stdlib `database/sql` pool; the driver is selected at runtime by the
/// connection string (no compile-time driver dependency).
const SQL_RUNTIME: &str = r#"type keelSQLPool struct {
	db *sql.DB
}

type keelSQLQueryResult struct {
	cols []string
	rows [][]any
}

type keelSQLRow struct {
	cols   []string
	values []any
}

type keelSQLRowMapper struct {
	result keelSQLQueryResult
	mapper func(keelSQLRow) any
}

func keelSQLErr(tag string, message string) KeelEnum {
	return KeelEnum{tag: tag, values: []any{message}}
}

func keelSQLConnect(conn string) KeelEnum {
	db, err := sql.Open(keelSQLDriver(conn), conn)
	if err != nil {
		return Err(keelSQLErr("ConnectionFailed", err.Error()))
	}
	return Ok(keelSQLPool{db: db})
}

func keelSQLDriver(conn string) string {
	if strings.HasPrefix(conn, "postgres://") || strings.HasPrefix(conn, "postgresql://") {
		return "postgres"
	}
	if strings.HasPrefix(conn, "mysql://") {
		return "mysql"
	}
	return "sqlite"
}

func keelSQLMigrate(pool any, statements string) KeelEnum {
	p := pool.(keelSQLPool)
	for _, raw := range strings.Split(statements, ";") {
		stmt := strings.TrimSpace(raw)
		if stmt == "" {
			continue
		}
		if _, err := p.db.Exec(stmt); err != nil {
			return Err(keelSQLErr("MigrationFailed", err.Error()))
		}
	}
	return Ok(struct{}{})
}

func keelSQLQuery(pool any, query string) KeelEnum {
	p := pool.(keelSQLPool)
	rows, err := p.db.Query(query)
	if err != nil {
		return Err(keelSQLErr("QueryFailed", err.Error()))
	}
	defer rows.Close()
	cols, err := rows.Columns()
	if err != nil {
		return Err(keelSQLErr("QueryFailed", err.Error()))
	}
	var data [][]any
	for rows.Next() {
		cells := make([]any, len(cols))
		ptrs := make([]any, len(cols))
		for i := range cells {
			ptrs[i] = &cells[i]
		}
		if err := rows.Scan(ptrs...); err != nil {
			return Err(keelSQLErr("QueryFailed", err.Error()))
		}
		data = append(data, cells)
	}
	return Ok(keelSQLQueryResult{cols: cols, rows: data})
}

func keelSQLQueryOne(pool any, query string) KeelEnum {
	res := keelSQLQuery(pool, query)
	if res.tag == "Err" {
		return res
	}
	qr := res.values[0].(keelSQLQueryResult)
	if len(qr.rows) == 0 {
		return Err(KeelEnum{tag: "NoRows"})
	}
	if len(qr.rows) > 1 {
		return Err(keelSQLErr("QueryFailed", "query_one matched multiple rows"))
	}
	return Ok(keelSQLRow{cols: qr.cols, values: qr.rows[0]})
}

func keelSQLExec(pool any, query string) KeelEnum {
	p := pool.(keelSQLPool)
	res, err := p.db.Exec(query)
	if err != nil {
		return Err(keelSQLErr("QueryFailed", err.Error()))
	}
	n, err := res.RowsAffected()
	if err != nil {
		return Err(keelSQLErr("QueryFailed", err.Error()))
	}
	return Ok(n)
}

func keelSQLMap(result any, mapper func(keelSQLRow) any) keelSQLRowMapper {
	return keelSQLRowMapper{result: result.(keelSQLQueryResult), mapper: mapper}
}

func keelSQLCollect(mapper any) KeelEnum {
	m := mapper.(keelSQLRowMapper)
	out := []any{}
	for _, values := range m.result.rows {
		out = append(out, m.mapper(keelSQLRow{cols: m.result.cols, values: values}))
	}
	return Ok(out)
}

func keelSQLRowGet(row any, index int64) any {
	r := row.(keelSQLRow)
	if index < 0 || int(index) >= len(r.values) {
		return nil
	}
	return r.values[index]
}

func keelSQLRowGetInt(row any, index int64) int64 {
	switch v := keelSQLRowGet(row, index).(type) {
	case int64:
		return v
	case int:
		return int64(v)
	default:
		return 0
	}
}

func keelSQLRowGetString(row any, index int64) string {
	switch v := keelSQLRowGet(row, index).(type) {
	case string:
		return v
	case []byte:
		return string(v)
	default:
		return ""
	}
}

func keelSQLRowGetBool(row any, index int64) bool {
	if v, ok := keelSQLRowGet(row, index).(bool); ok {
		return v
	}
	return false
}

func keelSQLRowGetFloat(row any, index int64) float64 {
	if v, ok := keelSQLRowGet(row, index).(float64); ok {
		return v
	}
	return 0
}

"#;

/// Router runtime for `http.serve(port, http.Router{...})` (KDR-0031). Emitted
/// once when a module calls `http.serve`. Splits each route pattern into method
/// and path, matches `{name}` path segments, and exposes typed path/query
/// parameter extraction. `K1505` (invalid port) is the runtime panic here.
const HTTP_ROUTER_RUNTIME: &str = r#"type keelRoute struct {
	pattern string
	handler func(keelHTTPRequest) keelHTTPResponse
}

func keelRouteParts(pattern string) (string, string) {
	fields := strings.Fields(pattern)
	if len(fields) >= 2 {
		return fields[0], fields[1]
	}
	if len(fields) == 1 {
		return "GET", fields[0]
	}
	return "GET", "/"
}

func keelRouteMatch(pattern string, method string, path string) (map[string]string, bool) {
	pMethod, pPath := keelRouteParts(pattern)
	if pMethod != method {
		return nil, false
	}
	pSegs := strings.Split(strings.Trim(pPath, "/"), "/")
	aSegs := strings.Split(strings.Trim(path, "/"), "/")
	if len(pSegs) != len(aSegs) {
		return nil, false
	}
	params := map[string]string{}
	for i := range pSegs {
		if strings.HasPrefix(pSegs[i], "{") && strings.HasSuffix(pSegs[i], "}") {
			params[pSegs[i][1:len(pSegs[i])-1]] = aSegs[i]
		} else if pSegs[i] != aSegs[i] {
			return nil, false
		}
	}
	return params, true
}

func keelHTTPServe(port int64, routes []keelRoute) KeelEnum {
	if port < 1 || port > 65535 {
		panic("invalid HTTP port")
	}
	mux := http.NewServeMux()
	mux.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) {
		body, _ := io.ReadAll(r.Body)
		for _, route := range routes {
			if params, ok := keelRouteMatch(route.pattern, r.Method, r.URL.Path); ok {
				req := keelHTTPRequest{body: string(body), method: r.Method, path: r.URL.Path, params: params, rawQuery: r.URL.RawQuery}
				resp := route.handler(req)
				w.WriteHeader(int(resp.status))
				_, _ = w.Write([]byte(resp.body))
				return
			}
		}
		w.WriteHeader(404)
	})
	err := http.ListenAndServe(fmt.Sprintf(":%d", port), mux)
	if err != nil {
		return Err(BindFailed(err.Error()))
	}
	return Ok(struct{}{})
}

func keelPathParamString(req keelHTTPRequest, name string) KeelEnum {
	v, ok := req.params[name]
	if !ok {
		return Err("missing path parameter: " + name)
	}
	return Ok(v)
}

func keelPathParamInt(req keelHTTPRequest, name string) KeelEnum {
	v, ok := req.params[name]
	if !ok {
		return Err("missing path parameter: " + name)
	}
	n, err := strconv.ParseInt(v, 10, 64)
	if err != nil {
		return Err("invalid integer for " + name + ": " + v)
	}
	return Ok(n)
}

func keelPathParamBool(req keelHTTPRequest, name string) KeelEnum {
	v, ok := req.params[name]
	if !ok {
		return Err("missing path parameter: " + name)
	}
	b, err := strconv.ParseBool(v)
	if err != nil {
		return Err("invalid bool for " + name + ": " + v)
	}
	return Ok(b)
}

func keelPathParamFloat(req keelHTTPRequest, name string) KeelEnum {
	v, ok := req.params[name]
	if !ok {
		return Err("missing path parameter: " + name)
	}
	f, err := strconv.ParseFloat(v, 64)
	if err != nil {
		return Err("invalid float for " + name + ": " + v)
	}
	return Ok(f)
}

func keelPathParamUuid(req keelHTTPRequest, name string) KeelEnum {
	v, ok := req.params[name]
	if !ok {
		return Err("missing path parameter: " + name)
	}
	if !keelUUIDValid(v) {
		return Err("invalid UUID for " + name + ": " + v)
	}
	return Ok(v)
}

func keelPathParamTimestamp(req keelHTTPRequest, name string) KeelEnum {
	v, ok := req.params[name]
	if !ok {
		return Err("missing path parameter: " + name)
	}
	parsed, ok := keelTimestampParse(v)
	if !ok {
		return Err("invalid timestamp for " + name + ": " + v)
	}
	return Ok(parsed)
}

func keelPathParamEmail(req keelHTTPRequest, name string) KeelEnum {
	v, ok := req.params[name]
	if !ok {
		return Err("missing path parameter: " + name)
	}
	parsed, ok := keelEmailParse(v)
	if !ok {
		return Err("invalid email for " + name + ": " + v)
	}
	return Ok(parsed)
}

func keelQueryValues(req keelHTTPRequest) url.Values {
	values, err := url.ParseQuery(req.rawQuery)
	if err != nil {
		return url.Values{}
	}
	return values
}

func keelQueryParamString(req keelHTTPRequest, name string) KeelEnum {
	values := keelQueryValues(req)
	if !values.Has(name) {
		return None
	}
	return Some(values.Get(name))
}

func keelQueryParamInt(req keelHTTPRequest, name string) KeelEnum {
	values := keelQueryValues(req)
	if !values.Has(name) {
		return None
	}
	n, err := strconv.ParseInt(values.Get(name), 10, 64)
	if err != nil {
		return None
	}
	return Some(n)
}

func keelQueryParamBool(req keelHTTPRequest, name string) KeelEnum {
	values := keelQueryValues(req)
	if !values.Has(name) {
		return None
	}
	b, err := strconv.ParseBool(values.Get(name))
	if err != nil {
		return None
	}
	return Some(b)
}

func keelQueryParamFloat(req keelHTTPRequest, name string) KeelEnum {
	values := keelQueryValues(req)
	if !values.Has(name) {
		return None
	}
	f, err := strconv.ParseFloat(values.Get(name), 64)
	if err != nil {
		return None
	}
	return Some(f)
}

func keelQueryParamUuid(req keelHTTPRequest, name string) KeelEnum {
	values := keelQueryValues(req)
	if !values.Has(name) {
		return None
	}
	v := values.Get(name)
	if !keelUUIDValid(v) {
		return None
	}
	return Some(v)
}

func keelQueryParamTimestamp(req keelHTTPRequest, name string) KeelEnum {
	values := keelQueryValues(req)
	if !values.Has(name) {
		return None
	}
	parsed, ok := keelTimestampParse(values.Get(name))
	if !ok {
		return None
	}
	return Some(parsed)
}

func keelQueryParamEmail(req keelHTTPRequest, name string) KeelEnum {
	values := keelQueryValues(req)
	if !values.Has(name) {
		return None
	}
	v := values.Get(name)
	parsed, ok := keelEmailParse(v)
	if !ok {
		return None
	}
	return Some(parsed)
}

"#;

/// `std.config` runtime (KDR-0030). The `Secret` marker type and the
/// `config.Error` constructors; per-struct loaders are generated separately
/// (they read the specific env vars). `keelConfigBool` follows the spec's
/// truthy/falsy table rather than `strconv.ParseBool`.
const CONFIG_RUNTIME: &str = r#"type keelConfigSecret struct {
	value string
}

func (s keelConfigSecret) unwrap() string {
	return s.value
}

func keelConfigMissingEnvVar(field string) KeelEnum {
	return KeelEnum{tag: "MissingEnvVar", values: []any{field}}
}

func keelConfigMissingSecret(field string) KeelEnum {
	return KeelEnum{tag: "MissingSecret", values: []any{field}}
}

func keelConfigParseError(field string, typ string, message string) KeelEnum {
	return KeelEnum{tag: "ParseError", values: []any{field, typ, message}}
}

func keelConfigBool(value string) (bool, bool) {
	switch value {
	case "true", "1", "yes", "on":
		return true, true
	case "false", "0", "no", "off":
		return false, true
	default:
		return false, false
	}
}

"#;

impl<'a> Emitter<'a> {
    pub(super) fn emit_runtime(&mut self) -> Result<(), BackendError> {
        self.line("type KeelEnum struct {")?;
        self.indent += 1;
        self.line("tag string")?;
        self.line("values []any")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("type keelTimestamp struct { seconds int64; nanos int32 }")?;
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
        if self.uses_json || self.uses_http || self.uses_uuid_new {
            self.emit_uuid_runtime()?;
        }
        if self.uses_json || self.uses_http || self.uses_timestamp_now {
            self.emit_timestamp_runtime()?;
        }
        if self.uses_json || self.uses_http {
            self.emit_email_runtime()?;
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
        if self.uses_sql {
            self.output.push_str(SQL_RUNTIME);
        }
        if self.uses_config {
            self.output.push_str(CONFIG_RUNTIME);
        }
        Ok(())
    }

    fn emit_uuid_runtime(&mut self) -> Result<(), BackendError> {
        self.line("func keelUUIDValid(value string) bool {")?;
        self.indent += 1;
        self.line("if len(value) != 36 { return false }")?;
        self.line("for i := 0; i < len(value); i++ {")?;
        self.indent += 1;
        self.line("if i == 8 || i == 13 || i == 18 || i == 23 { if value[i] != '-' { return false }; continue }")?;
        self.line("if !((value[i] >= '0' && value[i] <= '9') || (value[i] >= 'a' && value[i] <= 'f')) { return false }")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("return value[14] == '4' && (value[19] == '8' || value[19] == '9' || value[19] == 'a' || value[19] == 'b')")?;
        self.indent -= 1;
        self.line("}")?;
        if self.uses_uuid_new {
            self.line("func keelUUIDNew() string {")?;
            self.indent += 1;
            self.line("var value [16]byte")?;
            self.line("if _, err := rand.Read(value[:]); err != nil { panic(\"uuid random source unavailable: \" + err.Error()) }")?;
            self.line("value[6] = (value[6] & 0x0f) | 0x40")?;
            self.line("value[8] = (value[8] & 0x3f) | 0x80")?;
            self.line("return fmt.Sprintf(\"%x-%x-%x-%x-%x\", value[0:4], value[4:6], value[6:8], value[8:10], value[10:16])")?;
            self.indent -= 1;
            self.line("}")?;
        }
        self.line("")?;
        Ok(())
    }

    fn emit_timestamp_runtime(&mut self) -> Result<(), BackendError> {
        self.line("func keelTimestampFormat(value keelTimestamp) string {")?;
        self.indent += 1;
        self.line(
            "return time.Unix(value.seconds, int64(value.nanos)).UTC().Format(time.RFC3339Nano)",
        )?;
        self.indent -= 1;
        self.line("}")?;
        self.line("func keelTimestampSyntax(value string) bool {")?;
        self.indent += 1;
        self.line("if len(value) < 20 || value[4] != '-' || value[7] != '-' || value[10] != 'T' || value[13] != ':' || value[16] != ':' { return false }")?;
        self.line("for _, i := range []int{0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18} { if value[i] < '0' || value[i] > '9' { return false } }")?;
        self.line("i := 19")?;
        self.line("if value[i] == '.' {")?;
        self.indent += 1;
        self.line("i++")?;
        self.line("start := i")?;
        self.line("for i < len(value) && value[i] >= '0' && value[i] <= '9' { i++ }")?;
        self.line("if i == start || i-start > 9 { return false }")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("if i < len(value) && value[i] == 'Z' { return i+1 == len(value) }")?;
        self.line("if len(value)-i != 6 || (value[i] != '+' && value[i] != '-') || value[i+3] != ':' { return false }")?;
        self.line("for _, j := range []int{i + 1, i + 2, i + 4, i + 5} { if value[j] < '0' || value[j] > '9' { return false } }")?;
        self.line("hours := int(value[i+1]-'0')*10 + int(value[i+2]-'0')")?;
        self.line("minutes := int(value[i+4]-'0')*10 + int(value[i+5]-'0')")?;
        self.line("return hours <= 23 && minutes <= 59 && !(value[i] == '-' && hours == 0 && minutes == 0)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("func keelTimestampParse(value string) (keelTimestamp, bool) {")?;
        self.indent += 1;
        self.line("if !keelTimestampSyntax(value) { return keelTimestamp{}, false }")?;
        self.line("parsed, err := time.Parse(time.RFC3339Nano, value)")?;
        self.line("if err != nil { return keelTimestamp{}, false }")?;
        self.line("parsed = parsed.UTC()")?;
        self.line(
            "if parsed.Year() < 0 || parsed.Year() > 9999 { return keelTimestamp{}, false }",
        )?;
        self.line(
            "return keelTimestamp{seconds: parsed.Unix(), nanos: int32(parsed.Nanosecond())}, true",
        )?;
        self.indent -= 1;
        self.line("}")?;
        self.line("func keelTimestampNow() keelTimestamp {")?;
        self.indent += 1;
        self.line("now := time.Now().UTC()")?;
        self.line("return keelTimestamp{seconds: now.Unix(), nanos: int32(now.Nanosecond())}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        Ok(())
    }

    fn emit_email_runtime(&mut self) -> Result<(), BackendError> {
        self.line("func keelEmailValid(value string) bool {")?;
        self.indent += 1;
        self.line("if len(value) == 0 || len(value) > 254 { return false }")?;
        self.line("at := -1")?;
        self.line("for i := 0; i < len(value); i++ { if value[i] == '@' { if at >= 0 { return false }; at = i } }")?;
        self.line(
            "if at <= 0 || at > 64 || at == len(value)-1 || len(value)-at-1 > 253 { return false }",
        )?;
        self.line("atom := 0")?;
        self.line("for i := 0; i < at; i++ {")?;
        self.indent += 1;
        self.line("c := value[i]")?;
        self.line("if c == '.' { if atom == 0 { return false }; atom = 0; continue }")?;
        self.line("allowed := (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '!' || c == '#' || c == '$' || c == '%' || c == '&' || c == '\\'' || c == '*' || c == '+' || c == '-' || c == '/' || c == '=' || c == '?' || c == '^' || c == '_' || c == '`' || c == '{' || c == '|' || c == '}' || c == '~'")?;
        self.line("if !allowed { return false }")?;
        self.line("atom++")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("if atom == 0 { return false }")?;
        self.line("label := 0")?;
        self.line("for i := at + 1; i < len(value); i++ {")?;
        self.indent += 1;
        self.line("c := value[i]")?;
        self.line("if c == '.' { if label == 0 || value[i-1] == '-' { return false }; label = 0; continue }")?;
        self.line("if !((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || (c == '-' && label > 0)) { return false }")?;
        self.line("label++")?;
        self.line("if label > 63 { return false }")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("return label > 0 && value[len(value)-1] != '-'")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("func keelEmailParse(value string) (string, bool) {")?;
        self.indent += 1;
        self.line("if !keelEmailValid(value) { return \"\", false }")?;
        self.line("canonical := []byte(value)")?;
        self.line("domain := false")?;
        self.line("for i := 0; i < len(canonical); i++ {")?;
        self.indent += 1;
        self.line("if canonical[i] == '@' { domain = true; continue }")?;
        self.line(
            "if domain && canonical[i] >= 'A' && canonical[i] <= 'Z' { canonical[i] += 'a' - 'A' }",
        )?;
        self.indent -= 1;
        self.line("}")?;
        self.line("return string(canonical), true")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
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
        self.line("params map[string]string")?;
        self.line("rawQuery string")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func BindFailed(message string) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"BindFailed\", values: []any{message}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelString(v any) string {")?;
        self.indent += 1;
        self.line("if s, ok := v.(string); ok {")?;
        self.indent += 1;
        self.line("return s")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("return fmt.Sprint(v)")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        if self.uses_http_serve {
            self.output.push_str(HTTP_ROUTER_RUNTIME);
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
