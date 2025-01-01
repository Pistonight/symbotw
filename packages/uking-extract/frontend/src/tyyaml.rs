use std::marker::PhantomData;

use serde::{de::Visitor, Deserialize};
use serde_json::json;
use serde_yaml_ng::Value;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct DataSheet {
    pub enums: Vec<Enum>,
    pub structs: Vec<Struct>,
    pub unions: Vec<Union>,
    pub addresses: OrderedMap<Address>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Enum {
    pub name: String,
    pub size: usize,
    #[serde(default)]
    pub enumerators: OrderedMap<i128>,
}

impl Enum {
    pub fn emit_python(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!("ti.add_enum(\"{}\",", self.name));
        out.push(format!("    EnumDef(0x{:x}, [", self.size));
        for (name, value) in self.enumerators.iter() {
            if *value < 0 {
                out.push(format!("        (\"{}\", {}),", name, value));
            } else {
                out.push(format!("        (\"{}\", 0x{:x}),", name, value));
            }
        }
        if let Some(x) = out.last_mut() {
            if x.ends_with(",") {
                x.pop();
            }
        }
        if self.enumerators.is_empty() {
            out.last_mut().unwrap().push_str("]))");
        } else {
            out.push("    ]))".to_string());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Struct {
    pub name: String,
    pub size: usize,
    pub align: usize,
    #[serde(rename = "members", default)]
    pub off2member: OrderedMap<Member>,
    #[serde(default)]
    pub vtable: Vec<Member>,
}

impl Struct {
    pub fn emit_python(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!("ti.add_struct(\"{}\",", self.name));
        out.push(format!(
            "    StructDef(0x{:x}, 0x{:x}, [",
            self.size, self.align
        ));
        for (offset, member) in self.off2member.iter() {
            out.push(format!("        {},", member.emit_python(offset)));
        }
        if let Some(x) = out.last_mut() {
            if x.ends_with(",") {
                x.pop();
            }
        }
        if self.off2member.is_empty() {
            out.last_mut().unwrap().push_str("]), [");
        } else {
            out.push("    ]), [".to_string());
        }
        if self.vtable.is_empty() {
            out.last_mut().unwrap().push_str("])");
        } else {
            for member in &self.vtable {
                let name = &member.name;
                let tyyaml = emit_tyyaml_python(&member.tyyaml);
                out.push(format!("        (\"{}\", {}),", name, tyyaml));
            }
            if let Some(x) = out.last_mut() {
                if x.ends_with(",") {
                    x.pop();
                }
            }
            out.push("    ])".to_string());
        }
        out
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Member {
    pub name: String,
    #[serde(rename = "type")]
    pub tyyaml: Value,
    #[serde(default)]
    pub base: bool,
}

impl Member {
    pub fn emit_python(&self, offset: &str) -> String {
        if self.base {
            format!(
                "_make_member(\"{}\", {}, {}, True)",
                self.name,
                offset,
                emit_tyyaml_python(&self.tyyaml)
            )
        } else {
            format!(
                "_make_member(\"{}\", {}, {})",
                self.name,
                offset,
                emit_tyyaml_python(&self.tyyaml)
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Union {
    pub name: String,
    pub align: usize,
    pub size: usize,
    #[serde(default)]
    pub members: OrderedMap<Value>,
}

impl Union {
    pub fn emit_python(&self) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!("ti.add_union(\"{}\",", self.name));
        out.push(format!(
            "    UnionDef(0x{:x}, 0x{:x}, [",
            self.size, self.align
        ));
        for (name, tyyaml) in self.members.iter() {
            out.push(format!(
                "        _make_union_member(\"{}\", {}),",
                name,
                emit_tyyaml_python(tyyaml)
            ));
        }
        if let Some(x) = out.last_mut() {
            if x.ends_with(",") {
                x.pop();
            }
        }
        if self.members.is_empty() {
            out.last_mut().unwrap().push_str("]))");
        } else {
            out.push("    ]))".to_string());
        }
        out
    }
}

fn emit_tyyaml_python(tyyaml: &Value) -> String {
    // convert value to json
    let json_str: String = serde_json::to_string(tyyaml).unwrap();
    // convert string to json
    let json_str_str = serde_json::to_string(&json!(json_str)).unwrap();

    format!("json.loads({})", json_str_str)
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Address {
    Function(Function),
    Data(Data),
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Function {
    #[serde(rename = "func")]
    pub name: String,
    #[serde(rename = "type")]
    pub tyyaml: Option<Value>,
    #[serde(default)]
    pub args: Vec<Arg>,
}

impl Function {
    pub fn emit_python(&self, offset: &str) -> Vec<String> {
        let mut out = Vec::new();
        out.push(format!("ai.add_func({},", offset));
        out.push(format!("    _make_function(\"{}\",", self.name));
        if let Some(tyyaml) = &self.tyyaml {
            out.last_mut()
                .unwrap()
                .push_str(&format!("{}, [", emit_tyyaml_python(tyyaml)));
        } else {
            out.last_mut().unwrap().push_str("[], [");
        }
        if self.args.is_empty() {
            out.last_mut().unwrap().push_str("]))");
        } else {
            for arg in &self.args {
                out.push(format!("        {},", arg.emit_python()));
            }
            if let Some(x) = out.last_mut() {
                if x.ends_with(",") {
                    x.pop();
                }
            }
            out.push("    ]))".to_string());
        }
        out
    }
}
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Arg {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub tyyaml: Option<Value>,
}
impl Arg {
    pub fn emit_python(&self) -> String {
        let name = self.name.as_deref().unwrap_or("");
        if let Some(tyyaml) = &self.tyyaml {
            format!(
                "_make_name_type(\"{}\", {})",
                name,
                emit_tyyaml_python(tyyaml)
            )
        } else {
            format!("_make_name_type(\"{}\", [])", name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Data {
    #[serde(rename = "data")]
    pub name: String,
    #[serde(rename = "type")]
    pub tyyaml: Option<Value>,
}
impl Data {
    pub fn emit_python(&self, offset: &str) -> String {
        let name_type = Arg {
            name: Some(self.name.clone()),
            tyyaml: self.tyyaml.clone(),
        };
        format!("ai.add_data({}, {})", offset, name_type.emit_python())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct OrderedMap<T>(Vec<(String, T)>);
impl<'de, T: Deserialize<'de>> Deserialize<'de> for OrderedMap<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V<T>(PhantomData<T>);
        impl<'de, T: Deserialize<'de>> Visitor<'de> for V<T> {
            type Value = OrderedMap<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "a map")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut v = match map.size_hint() {
                    Some(size) => Vec::with_capacity(size),
                    None => Vec::new(),
                };
                while let Some((k, value)) = map.next_entry::<String, T>()? {
                    v.push((k, value));
                }

                Ok(OrderedMap(v))
            }
        }
        deserializer.deserialize_map(V(PhantomData))
    }
}

impl<T> std::ops::Deref for OrderedMap<T> {
    type Target = Vec<(String, T)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> std::ops::DerefMut for OrderedMap<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T> Default for OrderedMap<T> {
    fn default() -> Self {
        OrderedMap(Vec::new())
    }
}
