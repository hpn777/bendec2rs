// Ported from Bendec 0.52.0, Copyright (c) 2020 Daniel Fudala.
// MIT License
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

use serde_json::{json, Value};

pub(crate) type EmitResult = Result<String, String>;

pub(crate) fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        Value::Array(a) => a
            .iter()
            .map(|x| if x.is_null() { String::new() } else { text(x) })
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".into(),
        _ => v.to_string(),
    }
}

pub(crate) fn string<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{key} must be a string"))
}

pub(crate) fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value], String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("{key} must be an array"))
}

pub(crate) fn truth(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

pub(crate) fn smoosh(parts: Vec<String>) -> String {
    parts
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn doc(v: &Value, indent: usize) -> String {
    if v.is_null() {
        String::new()
    } else {
        format!("{}/// {}", " ".repeat(indent), text(v))
    }
}

fn strings(v: &Value, context: &str) -> Result<Vec<String>, String> {
    if v.is_null() {
        return Ok(vec![]);
    }
    v.as_array()
        .ok_or_else(|| format!("{context} must be an array of strings"))?
        .iter()
        .map(|s| {
            s.as_str()
                .map(str::to_owned)
                .ok_or_else(|| format!("{context} must contain strings"))
        })
        .collect()
}

pub(crate) fn annotations(meta: &Value) -> EmitResult {
    Ok(strings(&meta["annotations"], "annotations")?.join("\n"))
}

fn derives(values: &[String]) -> String {
    if values.is_empty() {
        String::new()
    } else {
        format!("#[derive({})]", values.join(", "))
    }
}

pub(crate) fn template(source: &str, bindings: &[(&str, String)]) -> EmitResult {
    // Deliberately no eval, function parsing, or Lodash's arbitrary JS expressions.
    if source.contains("<%") {
        return Err("JavaScript template evaluation is not portable".into());
    }
    let mut result = String::new();
    let mut tail = source;
    while let Some(start) = tail.find("{{") {
        result.push_str(&tail[..start]);
        let rest = &tail[start + 2..];
        let end = rest.find("}}").ok_or("unterminated template placeholder")?;
        let key = rest[..end].trim();
        let value = bindings.iter().find(|(k, _)| *k == key)
            .ok_or_else(|| format!("unsupported template expression {{{{ {key} }}}}; JavaScript callbacks/expressions are not portable"))?;
        result.push_str(&value.1);
        tail = &rest[end + 2..];
    }
    result.push_str(tail);
    Ok(result)
}

pub(crate) fn kind(v: &Value) -> Result<u8, String> {
    for (key, kind) in [
        ("size", 1),
        ("alias", 2),
        ("fields", 3),
        ("variants", 4),
        ("members", 5),
    ] {
        // Array detection precedes alias/struct in upstream normalization.
        if key == "alias" && v.get("type").is_some() && v.get("length").is_some() {
            return Ok(6);
        }
        if v.get(key).is_some() {
            return Ok(kind);
        }
    }
    if v.get("type").is_some() && v.get("length").is_some() {
        return Ok(6);
    }
    Err(format!("Unrecognized type definition: {v}"))
}

fn parse_int(v: &Value) -> Result<f64, String> {
    let s = text(v);
    let s = s.trim_start();
    let (sign, s) = if let Some(s) = s.strip_prefix('-') {
        (-1.0, s)
    } else {
        (1.0, s.strip_prefix('+').unwrap_or(s))
    };
    let (radix, s) = if s.starts_with("0x") || s.starts_with("0X") {
        (16, &s[2..])
    } else {
        (10, s)
    };
    let mut n = 0.0;
    let mut any = false;
    for c in s.chars() {
        if let Some(d) = c.to_digit(radix) {
            any = true;
            n = n * radix as f64 + d as f64;
        } else {
            break;
        }
    }
    if any {
        Ok(sign * n)
    } else {
        Err(format!("invalid numeric offset {s:?}"))
    }
}

pub(crate) fn variants(v: &Value) -> Result<Vec<(String, f64, Value)>, String> {
    let offset = if v.get("offset").is_some() {
        parse_int(&v["offset"])?
    } else {
        0.0
    };
    array(v, "variants")?
        .iter()
        .map(|variant| {
            let a = variant.as_array().ok_or("enum variant must be an array")?;
            let name = a
                .first()
                .and_then(Value::as_str)
                .ok_or("enum variant name must be a string")?
                .to_owned();
            let value = a.get(1).ok_or("enum variant needs a numeric value")?;
            let n = if let Some(s) = value.as_str() {
                if s.starts_with("0x") {
                    parse_int(value)?
                } else if let Some(s) = s.strip_prefix("0b") {
                    let digits: String = s.chars().take_while(|c| *c == '0' || *c == '1').collect();
                    if digits.is_empty() {
                        return Err("invalid binary enum value".into());
                    }
                    digits
                        .chars()
                        .fold(0.0, |n, c| n * 2.0 + if c == '1' { 1.0 } else { 0.0 })
                } else {
                    0.0
                } // Yes: upstream does not parse decimal strings here.
            } else {
                value
                    .as_f64()
                    .ok_or("enum variant value must be a number or radix string")?
            };
            if !n.is_finite() || !(n + offset).is_finite() {
                return Err("non-finite enum value is not supported".into());
            }
            Ok((name, n + offset, a.get(2).cloned().unwrap_or(Value::Null)))
        })
        .collect()
}

fn radix(n: f64, radix: u32) -> EmitResult {
    if !n.is_finite() || n.abs() >= u128::MAX as f64 {
        return Err("numeric value outside portable radix conversion range".into());
    }
    let mut integer = n.abs().trunc() as u128;
    let mut digits = Vec::new();
    loop {
        digits.push(char::from_digit((integer % radix as u128) as u32, radix).unwrap());
        integer /= radix as u128;
        if integer == 0 {
            break;
        }
    }
    digits.reverse();
    let mut s: String = digits.into_iter().collect();
    let mut frac = n.abs().fract();
    if frac != 0.0 {
        s.push('.');
        while frac != 0.0 {
            frac *= radix as f64;
            s.push(char::from_digit(frac.trunc() as u32, radix).unwrap());
            frac = frac.fract();
        }
    }
    if n < 0.0 {
        s.insert(0, '-');
    }
    Ok(s)
}

pub(crate) fn hex(n: f64, digits: usize) -> EmitResult {
    let s = format!("{}{}", "0".repeat(digits), radix(n, 16)?);
    Ok(format!("0x{}", &s[s.len().saturating_sub(digits)..]))
}

// Lodash's ASCII word boundaries (acronyms and digit groups included), plus
// common Latin deburring. No regex, locale, or third-party dependency is needed.
pub(crate) fn snake(s: &str) -> String {
    let mut normalized = String::new();
    for c in s.chars() {
        let replacement = match c {
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => "A",
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => "a",
            'Æ' => "Ae",
            'æ' => "ae",
            'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
            'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
            'Ð' | 'Ď' | 'Đ' => "D",
            'ð' | 'ď' | 'đ' => "d",
            'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
            'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
            'Ĥ' | 'Ħ' => "H",
            'ĥ' | 'ħ' => "h",
            'Ĳ' => "IJ",
            'ĳ' => "ij",
            'Ĵ' => "J",
            'ĵ' => "j",
            'Ķ' => "K",
            'ķ' | 'ĸ' => "k",
            'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => "L",
            'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
            'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
            'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
            'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
            'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
            'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
            'ñ' | 'ń' | 'ņ' | 'ň' => "n",
            'ŉ' => "n",
            'Ŋ' => "N",
            'ŋ' => "n",
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "O",
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
            'Œ' => "Oe",
            'œ' => "oe",
            'ß' => "ss",
            'ſ' => "s",
            'Ŕ' | 'Ŗ' | 'Ř' => "R",
            'ŕ' | 'ŗ' | 'ř' => "r",
            'Ś' | 'Ŝ' | 'Ş' | 'Š' => "S",
            'ś' | 'ŝ' | 'ş' | 'š' => "s",
            'Ţ' | 'Ť' | 'Ŧ' => "T",
            'ţ' | 'ť' | 'ŧ' => "t",
            'Ŵ' => "W",
            'ŵ' => "w",
            'Ź' | 'Ż' | 'Ž' => "Z",
            'ź' | 'ż' | 'ž' => "z",
            'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
            'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
            'Ý' | 'Ÿ' | 'Ŷ' => "Y",
            'ý' | 'ÿ' | 'ŷ' => "y",
            'Þ' => "Th",
            'þ' => "th",
            '\'' | '’' | '\u{0300}'..='\u{036f}' => "",
            _ => {
                normalized.push(c);
                continue;
            }
        };
        normalized.push_str(replacement);
    }
    let chars: Vec<char> = normalized.chars().collect();
    let mut words = Vec::new();
    let mut word = String::new();
    let mut ordinal_end = None;
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !word.is_empty() {
                words.push(std::mem::take(&mut word));
            }
            continue;
        }
        let prev = i.checked_sub(1).map(|p| chars[p]);
        let next = chars.get(i + 1).copied();
        let ordinal = prev.is_some_and(|p| p.is_ascii_digit())
            && chars.get(i + 1).is_some_and(|&second| {
                let last = prev.unwrap();
                let suffix = [c.to_ascii_lowercase(), second.to_ascii_lowercase()];
                let valid_suffix = match last {
                    '1' => suffix == ['s', 't'],
                    '2' => suffix == ['n', 'd'],
                    '3' => suffix == ['r', 'd'],
                    _ => suffix == ['t', 'h'],
                };
                let end = chars.get(i + 2).copied();
                let valid_end = end.is_none_or(|end| {
                    !end.is_alphanumeric()
                        || (c.is_lowercase() && end.is_uppercase())
                        || (c.is_uppercase() && end.is_lowercase())
                });
                valid_suffix && (c.is_ascii_lowercase() == second.is_ascii_lowercase()) && valid_end
            });
        if ordinal {
            // Lodash consumes the entire ordinal alternative as one word. In
            // particular, its final uppercase suffix must not be reinterpreted
            // as the start of an acronym before a following lowercase letter.
            ordinal_end = Some(i + 1);
        }
        let boundary = prev.is_some_and(|p| {
            p.is_alphanumeric()
                && ordinal_end != Some(i)
                && (ordinal_end == i.checked_sub(1)
                    || (!p.is_uppercase() && !p.is_ascii_digit() && c.is_uppercase())
                    || (p.is_ascii_digit() != c.is_ascii_digit() && !ordinal)
                    || (p.is_uppercase()
                        && c.is_uppercase()
                        && next.is_some_and(char::is_lowercase)))
        });
        if boundary && !word.is_empty() {
            words.push(std::mem::take(&mut word));
        }
        word.extend(c.to_lowercase());
    }
    if !word.is_empty() {
        words.push(word);
    }
    words.join("_")
}

fn check_keys(v: &Value, allowed: &[&str], context: &str) -> Result<(), String> {
    let map = v
        .as_object()
        .ok_or_else(|| format!("{context} must be an object"))?;
    for key in map.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unsupported {context} option: {key}"));
        }
    }
    Ok(())
}

fn validate_meta(meta: &Value) -> Result<(), String> {
    if meta.is_null() {
        return Ok(());
    }
    check_keys(
        meta,
        &[
            "annotations",
            "newtype",
            "fields",
            "bitflags",
            "implConst",
            "union",
            "publicFields",
            "privateFields",
        ],
        "type metadata",
    )?;
    for key in ["annotations", "publicFields", "privateFields"] {
        strings(&meta[key], key)?;
    }
    if !meta["fields"].is_null() {
        let fields = meta["fields"]
            .as_object()
            .ok_or("metadata fields must be an object")?;
        for field in fields.values() {
            check_keys(field, &["annotations"], "field metadata")?;
            strings(&field["annotations"], "field annotations")?;
        }
    }
    if !meta["newtype"].is_null() && truth(&meta["newtype"]) {
        let n = &meta["newtype"];
        check_keys(
            n,
            &["kind", "module", "constr", "intoInner", "asInner", "deref"],
            "newtype",
        )?;
        match string(n, "kind")? {
            "Public" | "Private" | "InCrate" => (),
            "InPath" => {
                string(n, "module")?;
            }
            other => return Err(format!("unsupported newtype kind: {other}")),
        }
    }
    if !meta["union"].is_null() && truth(&meta["union"]) {
        check_keys(
            &meta["union"],
            &["underlying", "discVariant", "discFn"],
            "union",
        )?;
        if meta["union"].get("discFn").is_some() {
            return Err("union.discFn JavaScript callbacks are not portable; omit it for identity discriminants".into());
        }
    }
    Ok(())
}

fn all_derives(options: &Value, name: &str, kind: &str) -> Result<Vec<String>, String> {
    let defaults: &[&str] = match kind {
        "struct" | "bitflags" => &["Serialize", "Deserialize"],
        "enum" => &[
            "Debug",
            "Copy",
            "Clone",
            "Eq",
            "PartialEq",
            "Serialize",
            "Deserialize",
        ],
        _ => &[],
    };
    let mut result = if let Some(d) = options.get("defaultDerives").and_then(|d| d.get(kind)) {
        strings(d, "defaultDerives")?
    } else {
        defaults.iter().map(|s| s.to_string()).collect()
    };
    let extra = &options["extraDerives"];
    let mut found = extra.get(name);
    // Lodash get supports dotted paths unless a literal key already exists.
    if found.is_none() {
        let mut current = extra;
        for section in name.split('.') {
            current = &current[section];
        }
        if !current.is_null() {
            found = Some(current);
        }
    }
    if let Some(d) = found {
        result.extend(strings(d, "extraDerives")?);
    }
    Ok(result)
}

fn alias(v: &Value, alias: &str, meta: &Value, options: &Value) -> EmitResult {
    let name = string(v, "name")?;
    let alias = alias.replace('.', "::");
    let newtype = &meta["newtype"];
    if !truth(newtype) {
        return Ok(smoosh(vec![
            doc(&v["description"], 0),
            format!("pub type {name} = {alias};"),
        ]));
    }
    let visibility = match string(newtype, "kind")? {
        "Public" => "pub ".into(),
        "Private" => "".into(),
        "InCrate" => "pub(crate) ".into(),
        "InPath" => format!("pub(in {}) ", string(newtype, "module")?),
        other => return Err(format!("unsupported newtype kind {other}")),
    };
    let mut body = vec![format!("pub struct {name}({visibility}{alias});")];
    if truth(&newtype["constr"]) {
        body.push(format!(
            "impl {name} {{\n  pub fn new(v: {alias}) -> Self {{\n    Self(v)\n  }}\n}}"
        ));
    }
    if truth(&newtype["intoInner"]) {
        body.push(format!(
            "impl {name} {{\n  pub fn into_inner(self) -> {alias} {{\n    self.0\n  }}\n}}"
        ));
    }
    if truth(&newtype["asInner"]) {
        body.push(format!(
            "impl {name} {{\n  pub fn as_inner(&self) -> {alias} {{\n    self.0\n  }}\n}}"
        ));
    }
    if truth(&newtype["deref"]) {
        body.push(format!("impl std::ops::Deref for {name} {{\n  type Target = {alias};\n  fn deref(&self) -> &Self::Target {{\n    &self.0\n  }}\n}}"));
    }
    Ok(smoosh(vec![
        doc(&v["description"], 0),
        derives(&all_derives(options, name, "newtype")?),
        smoosh(body),
    ]))
}

fn mapped(
    options: &Value,
    key: &str,
    size: Option<&Value>,
    name: &str,
    type_definition: bool,
) -> Result<Option<String>, String> {
    if let Some(value) = options.get("typeMapping").and_then(|v| v.get(key)) {
        let source = value.as_str().ok_or_else(|| {
            format!("typeMapping[{key}] must be a declarative string, not a JavaScript callback")
        })?;
        return template(
            source,
            &[
                ("size", size.map(text).unwrap_or_else(|| "undefined".into())),
                ("name", name.into()),
            ],
        )
        .map(Some);
    }
    if key == "char[]" {
        return Ok(Some(format!(
            "[u8; {}]",
            size.map(text).unwrap_or_else(|| "undefined".into())
        )));
    }
    // Upstream spreads mappings into an ordinary JS object, not a null-prototype
    // dictionary. Inherited Object.prototype methods are therefore invoked too.
    // Keep these observable default quirks without evaluating JavaScript. An
    // explicit declarative mapping above still overrides the inherited entry.
    let inherited = match key {
        "constructor" => size
            .filter(|value| !value.is_null())
            .map(text)
            .unwrap_or_else(|| "[object Object]".into()),
        "toString" | "toLocaleString" | "valueOf" => "[object Object]".into(),
        "hasOwnProperty" | "propertyIsEnumerable" => {
            let property = size.map(text).unwrap_or_else(|| "undefined".into());
            (property == "char[]"
                || options
                    .get("typeMapping")
                    .is_some_and(|mapping| mapping.get(&property).is_some()))
            .to_string()
        }
        "isPrototypeOf" => "false".into(),
        "__lookupGetter__" | "__lookupSetter__" => "undefined".into(),
        "__defineGetter__" | "__defineSetter__" => {
            return Err(format!("Object.prototype.{key}: Expecting function"));
        }
        "__proto__" => {
            return Err(if type_definition {
                "typeMap[typeName] is not a function"
            } else {
                "typeMap[key] is not a function"
            }
            .into());
        }
        _ => return Ok(None),
    };
    Ok(Some(inherited))
}

fn structure(v: &Value, meta: &Value, options: &Value) -> EmitResult {
    let name = string(v, "name")?;
    let all = all_derives(options, name, "struct")?;
    let public = if meta["publicFields"].is_null() {
        None
    } else {
        Some(strings(&meta["publicFields"], "publicFields")?)
    };
    let private = strings(&meta["privateFields"], "privateFields")?;
    let prefix = options["visibilityPrefix"].as_str().unwrap_or("");
    let mut members = Vec::new();
    for field in array(v, "fields")? {
        let field_name = string(field, "name")?;
        let field_type = string(field, "type")?.replace('.', "::");
        let is_array = truth(&field["length"]);
        let key = format!("{field_type}{}", if is_array { "[]" } else { "" });
        let final_type =
            mapped(options, &key, field.get("length"), name, false)?.unwrap_or_else(|| {
                if is_array {
                    format!("[{field_type}; {}]", text(&field["length"]))
                } else {
                    field_type
                }
            });
        let visibility = if private.iter().any(|s| s == field_name)
            || public
                .as_ref()
                .is_some_and(|p| !p.iter().any(|s| s == field_name))
        {
            prefix
        } else {
            "pub "
        };
        let mut parts = vec![doc(&field["description"], 2)];
        for annotation in strings(
            &meta["fields"][field_name]["annotations"],
            "field annotations",
        )? {
            parts.push(format!("  {annotation}"));
        }
        parts.push(format!(
            "  {visibility}{}: {final_type},",
            snake(field_name)
        ));
        members.push(smoosh(parts));
    }
    Ok(smoosh(vec![
        doc(&v["description"], 0),
        "#[repr(C, packed)]".into(),
        derives(&all),
        annotations(meta)?,
        if all.iter().any(|s| s == "Default") {
            "#[serde(deny_unknown_fields, default)]"
        } else {
            "#[serde(deny_unknown_fields)]"
        }
        .into(),
        if truth(&options["camelCase"]) {
            "#[serde(rename_all = \"camelCase\")]".into()
        } else {
            String::new()
        },
        format!("pub struct {name} {{"),
        members.join("\n"),
        "}".into(),
    ]))
}

fn enumeration(v: &Value, meta: &Value, options: &Value) -> EmitResult {
    let name = string(v, "name")?;
    let underlying = string(v, "underlying")?;
    let variants = variants(v)?;
    if truth(&meta["bitflags"]) || truth(&v["bitflags"]) {
        let width = match underlying {
            "u8" => 8,
            "u16" => 16,
            _ => 32,
        };
        let max_name = variants
            .iter()
            .map(|(name, _, _)| snake(name).len())
            .max()
            .unwrap_or(0);
        let mut fields = Vec::new();
        for (key, value, docs) in &variants {
            let key = snake(key).to_uppercase();
            let bits = radix(*value, 2)?;
            fields.push(smoosh(vec![
                doc(docs, 0),
                format!(
                    "    const {key:padding$} = 0b{bits:0>width$};",
                    padding = max_name,
                    width = width
                ),
            ]));
        }
        return Ok(smoosh(vec![
            format!(
                "bitflags::bitflags! {{\n  {}\n  {}",
                doc(&v["description"], 0),
                derives(&all_derives(options, name, "bitflags")?)
            ),
            if truth(&options["transparentBitflags"]) {
                "  #[serde(transparent)]".into()
            } else {
                String::new()
            },
            format!(
                "  #[repr(transparent)]\n  pub struct {name}: {underlying} {{\n{}\n  }}\n}}",
                fields.join("\n")
            ),
        ]));
    }
    let first = &variants
        .first()
        .ok_or("non-bitflags enums must have at least one variant")?
        .0;
    let mut fields = Vec::new();
    let mut reverse = Vec::new();
    let mut constants = Vec::new();
    for (key, value, docs) in &variants {
        let value = hex(*value, 4)?;
        fields.push(smoosh(vec![doc(docs, 2), format!("  {key} = {value},")]));
        reverse.push(format!("      {value} => Ok(Self::{key}),"));
        constants.push(format!("  pub const {key}: {underlying} = {value};"));
    }
    let conversion = &options["enumConversionError"];
    let error_type = conversion
        .get("type")
        .map(|v| {
            v.as_str()
                .ok_or("enumConversionError.type must be a template string")
        })
        .transpose()?
        .unwrap_or("{{ underlying }}");
    let constructor = conversion
        .get("constructor")
        .map(|v| {
            v.as_str()
                .ok_or("enumConversionError.constructor must be a template string")
        })
        .transpose()?
        .unwrap_or("other");
    let bindings = [("underlying", underlying.into()), ("name", name.into())];
    let error_type = template(error_type, &bindings)?;
    let constructor = template(constructor, &bindings)?;
    let mut impls = vec![smoosh(vec![doc(&v["description"], 0), format!("#[repr({underlying})]\n{}\npub enum {name} {{\n{}\n}}", derives(&all_derives(options, name, "enum")?), fields.join("\n"))]),
        format!("impl Default for {name} {{\n  fn default() -> Self {{\n    Self::{first}\n  }}\n}}"),
        format!("impl std::convert::TryFrom<{underlying}> for {name} {{\n  type Error = {error_type};\n  fn try_from(value: {underlying}) -> Result<Self, Self::Error> {{\n    match value {{\n{}\n      other => Err({constructor}),\n    }}\n  }}\n}}", reverse.join("\n"))];
    if truth(&meta["implConst"]) {
        impls.push(format!("pub struct {name}Int;\n#[allow(non_upper_case_globals, dead_code)]\nimpl {name}Int {{\n{}\n}}\n", constants.join("\n")));
    }
    Ok(smoosh(impls))
}

pub(crate) fn generate(
    types: &[Value],
    options: &Value,
    metadata: &Value,
    hooks: &mut dyn crate::NativeHooks,
    type_hooks: bool,
) -> EmitResult {
    let empty = json!({});
    let options = if options.is_null() { &empty } else { options };
    check_keys(
        options,
        &[
            "lookupTypes",
            "extras",
            "defaultDerives",
            "extraDerives",
            "meta",
            "camelCase",
            "enumConversionError",
            "forEachType",
            "transparentBitflags",
            "visibilityPrefix",
            "typeMapping",
        ],
        "generator",
    )?;
    if let Some(d) = options.get("defaultDerives") {
        check_keys(
            d,
            &["struct", "enum", "bitflags", "newtype"],
            "defaultDerives",
        )?;
        for value in d.as_object().unwrap().values() {
            strings(value, "defaultDerives")?;
        }
    }
    if let Some(m) = options.get("meta") {
        for meta in m.as_object().ok_or("meta must be an object")?.values() {
            validate_meta(meta)?;
        }
    }
    if let Some(extra) = options.get("extraDerives") {
        fn validate_extra(value: &Value) -> Result<(), String> {
            if let Some(object) = value.as_object() {
                for entry in object.values() {
                    validate_extra(entry)?;
                }
                Ok(())
            } else {
                strings(value, "extraDerives")?;
                Ok(())
            }
        }
        if !extra.is_object() && !extra.is_array() {
            return Err("extraDerives must be a type-name map".into());
        }
        validate_extra(extra)?;
    }
    if let Some(m) = options.get("typeMapping") {
        for (key, value) in m.as_object().ok_or("typeMapping must be an object")? {
            let source = value.as_str().ok_or_else(|| format!("typeMapping[{key}] must be a declarative string; JavaScript callbacks are not portable"))?;
            template(
                source,
                &[("size", "undefined".into()), ("name", String::new())],
            )?;
        }
    }
    if let Some(v) = options.get("forEachType") {
        let source = v.as_str().ok_or("forEachType must be a declarative template string; JavaScript callbacks are not portable")?;
        let trimmed = source.trim_start();
        if trimmed.starts_with("function") || (trimmed.starts_with('(') && trimmed.contains("=>")) {
            return Err("forEachType cannot execute JavaScript callbacks; provide a declarative Rust source template".into());
        }
        template(
            source,
            &[
                ("generated", String::new()),
                ("name", String::new()),
                ("context.name", String::new()),
            ],
        )?;
    }
    if let Some(v) = options.get("enumConversionError") {
        check_keys(v, &["type", "constructor"], "enumConversionError")?;
        for value in v.as_object().unwrap().values() {
            template(
                value
                    .as_str()
                    .ok_or("enumConversionError values must be strings")?,
                &[("underlying", String::new()), ("name", String::new())],
            )?;
        }
    }
    if let Some(lookup) = options.get("lookupTypes") {
        for entry in lookup.as_array().ok_or("lookupTypes must be an array")? {
            if let Some(a) = entry.as_array() {
                for t in a {
                    kind(t)?;
                }
            } else {
                kind(entry)?;
            }
        }
    }
    if let Some(prefix) = options.get("visibilityPrefix") {
        if !prefix.is_string() {
            return Err("visibilityPrefix must be a string".into());
        }
    }
    let mut definitions = Vec::new();
    for v in types {
        let kind = kind(v)?;
        let name = string(v, "name")?;
        let meta = &options["meta"][name];
        let generated = if let Some(mapped) = mapped(options, name, None, name, true)? {
            format!("pub type {name} = {mapped};")
        } else if name == "char" {
            "// ignored: char".into()
        } else {
            match kind {
                1 => format!("// primitive built-in: {name}"),
                2 => alias(v, string(v, "alias")?, meta, options)?,
                3 => structure(v, meta, options)?,
                4 => enumeration(v, meta, options)?,
                5 => crate::emitter_union::union(v, types, meta, hooks)?,
                6 => alias(
                    v,
                    &format!(
                        "[{}; {}]",
                        string(v, "type")?.replace('.', "::"),
                        text(&v["length"])
                    ),
                    meta,
                    options,
                )?,
                _ => unreachable!(),
            }
        };
        let generated = if let Some(source) = options.get("forEachType").and_then(Value::as_str) {
            template(
                source,
                &[
                    ("generated", generated),
                    ("name", name.into()),
                    ("context.name", name.into()),
                ],
            )?
        } else {
            generated
        };
        if type_hooks {
            let mut context = v.clone();
            context["kind"] = json!(kind);
            if kind == 4 {
                context["variants"] = Value::Array(
                    variants(v)?
                        .into_iter()
                        .map(|(n, v, d)| json!([n, v, d]))
                        .collect(),
                );
                context["bitflags"] = json!(truth(&v["bitflags"]));
            }
            definitions.push(hooks.for_each_type(generated, &context, &metadata[name])?);
        } else {
            definitions.push(generated);
        }
    }
    Ok(smoosh(vec!["/** GENERATED BY BENDEC TYPE GENERATOR */\n#[allow(unused_imports)]\nuse serde::{Deserialize, Deserializer, Serialize, Serializer};".into(), strings(&options["extras"], "extras")?.join("\n"), definitions.join("\n\n")]))
}
