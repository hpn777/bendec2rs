// Derived from Bendec 0.52.0. Copyright (c) 2020 Daniel Fudala.
// MIT licensed; full upstream permission and warranty notice in emitter.rs.

use crate::emitter::*;
use serde_json::Value;

fn find<'a>(types: &'a [Value], name: &str) -> Result<&'a Value, String> {
    types
        .iter()
        .find(|t| t["name"].as_str() == Some(name))
        .ok_or_else(|| format!("type {name} not found"))
}

pub(crate) fn union(
    v: &Value,
    types: &[Value],
    meta: &Value,
    hooks: &mut dyn crate::NativeHooks,
) -> EmitResult {
    let name = string(v, "name")?;
    let members: Vec<&str> = array(v, "members")?
        .iter()
        .map(|m| {
            m.as_str()
                .ok_or_else(|| "union members must be strings".into())
        })
        .collect::<Result<_, String>>()?;
    if truth(&meta["union"]) {
        let disc = find(types, string(&meta["union"], "discVariant")?)?;
        if kind(disc)? != 4 {
            return Err(format!(
                "type {} is not an enum",
                text(&meta["union"]["discVariant"])
            ));
        }
        return union_enum(v, &members, disc, meta, hooks);
    }
    let first = members
        .first()
        .ok_or("union must have at least one member")?;
    let mut disc = find(types, first)?;
    let path: Vec<&str> = array(v, "discriminator")?
        .iter()
        .map(|s| {
            s.as_str()
                .ok_or_else(|| "union discriminator path must contain strings".into())
        })
        .collect::<Result<_, String>>()?;
    for section in &path {
        if kind(disc)? != 3 {
            return Err(format!(
                "The path to union discriminator can only contain Structs, {} is not a Struct",
                text(&disc["name"])
            ));
        }
        let field = array(disc, "fields")?
            .iter()
            .find(|f| f["name"].as_str() == Some(section))
            .ok_or_else(|| format!("no field '{section}' in struct '{}'", text(&disc["name"])))?;
        disc = find(types, string(field, "type")?)?;
    }
    let disc_name = string(disc, "name")?;
    let fields = members
        .iter()
        .map(|m| format!("  pub {}: {m},", snake(m)))
        .collect::<Vec<_>>()
        .join("\n");
    let body = format!(
        "{}\npub union {name} {{\n{fields}\n}}",
        doc(&v["description"], 0)
    );
    let serialize = members
        .iter()
        .map(|m| {
            format!(
                "        {disc_name}::{m} => self.{}.serialize(serializer),",
                snake(m)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let disc_path = path.iter().map(|s| snake(s)).collect::<Vec<_>>().join(".");
    let serialize = format!("impl Serialize for {name} {{\n  fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>\n  where\n    S: Serializer,\n  {{\n    unsafe {{\n      match self.{}.{disc_path} {{\n{serialize} \n      }}\n    }}\n  }}\n}}", snake(first));
    let deserialize = members
        .iter()
        .map(|m| {
            format!(
                "      {disc_name}::{m} => {m}::deserialize(de).map(|v| {name} {{ {}: v }}),",
                snake(m)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let deserialize = format!("impl {name} {{\n  pub fn deserialize<'de, D>(de: D, disc: {disc_name}) -> Result<Self, D::Error>\n  where\n    D: Deserializer<'de>,\n  {{\n    match disc {{\n{deserialize}\n    }}\n  }}\n}}");
    let sizes = members
        .iter()
        .map(|m| format!("      {disc_name}::{m} => std::mem::size_of::<{m}>(),"))
        .collect::<Vec<_>>()
        .join("\n");
    let sizes = format!("impl {name} {{\n  pub const fn size_of(disc: {disc_name}) -> usize {{\n    match disc {{\n{sizes}\n    }}\n  }}\n}}");
    Ok([body, serialize, deserialize, sizes].join("\n\n"))
}

fn union_enum(
    v: &Value,
    members: &[&str],
    disc: &Value,
    meta: &Value,
    hooks: &mut dyn crate::NativeHooks,
) -> EmitResult {
    let name = string(v, "name")?;
    let disc_name = string(disc, "name")?;
    let underlying = meta["union"]["underlying"].as_str().unwrap_or("u8");
    // JS's default argument to hexPad is four when the lookup is undefined.
    let digits = match underlying {
        "u8" => 2,
        "u16" => 4,
        "u32" => 8,
        "u64" => 16,
        _ => 4,
    };
    let variants = variants(disc)?;
    let mut values = Vec::new();
    let mut comments = Vec::new();
    let mut fields = Vec::new();
    for member in members {
        let value = variants
            .iter()
            .rev()
            .find(|(n, _, _)| n == member)
            .ok_or_else(|| {
                format!(
                    "There is no {member} in enum {disc_name}\nPossible variants: {}",
                    variants
                        .iter()
                        .map(|(n, _, _)| n.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                )
            })?
            .1;
        let mut context = v.clone();
        context["kind"] = serde_json::json!(5);
        comments.push(hex(value, digits)?);
        let value = hex(hooks.union_discriminant(&context, member, value)?, digits)?;
        fields.push(format!("  {member}({member}) = {value},"));
        values.push(value);
    }
    let longest = fields.iter().map(String::len).max().unwrap_or(0);
    let fields = fields
        .iter()
        .zip(&comments)
        .map(|(field, value)| format!("{field:longest$} // {value}"))
        .collect::<Vec<_>>()
        .join("\n");
    let body = smoosh(vec![
        doc(&v["description"], 0),
        format!("#[repr({underlying})]"),
        annotations(meta)?,
        "#[derive(Serialize)]".into(),
        "#[serde(untagged)]".into(),
        format!("pub enum {name} {{"),
        format!("  {fields}"),
        "} ".into(),
    ]);
    let deserialize = members
        .iter()
        .map(|m| format!("      {disc_name}::{m} => {m}::deserialize(de).map(Self::{m}), "))
        .collect::<Vec<_>>()
        .join("\n");
    let deserialize = format!("impl {name} {{\n  pub fn deserialize<'de, D>(de: D, disc: {disc_name}) -> Result<Self, D::Error>\n  where\n  D: Deserializer<'de>,\n  {{\n    match disc {{\n      {deserialize}\n    }}\n  }}\n}} ");
    let mut impls = vec![body, deserialize];
    if truth(&meta["implConst"]) {
        let constants = members
            .iter()
            .zip(&values)
            .map(|(member, value)| format!("  pub const {member}: {underlying} = {value};"))
            .collect::<Vec<_>>()
            .join("\n");
        impls.push(format!("pub struct {name}Int;\n  #[allow(non_upper_case_globals, dead_code)]\n  impl {name}Int {{\n  {constants}\n  }}\n  "));
    }
    Ok(impls.join("\n\n"))
}
