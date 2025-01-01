use super::{TypePrim, TypeYaml};

/// Exportable type definition
#[derive(Debug, Clone, PartialEq)]
pub enum TypeDef {
    /// Struct or Class
    Struct(StructDef),
    /// Enum
    Enum(EnumDef),
    /// Union
    Union(UnionDef),
}

impl TypeDef {
    /// Create a PTMF type for the underlying type
    pub fn ptmf(name: String) -> Self {
        let func = MemberDef {
            offset: 0,
            name: "func".to_string(),
            is_base: false,
            ty_yaml: TypePrim::U64.yaml_string(),
        };
        let adjustment = MemberDef {
            offset: 8,
            name: "adjustment".to_string(),
            is_base: false,
            ty_yaml: TypePrim::I64.yaml_string(),
        };
        let members = vec![func, adjustment];
        let ptmf = StructDef {
            name,
            size: 16,
            alignment: 1,
            vtable: vec![],
            members,
        };
        Self::Struct(ptmf)
    }

    pub fn as_enum(&self) -> Option<&EnumDef> {
        match self {
            Self::Enum(e) => Some(e),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&StructDef> {
        match self {
            Self::Struct(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_union(&self) -> Option<&UnionDef> {
        match self {
            Self::Union(u) => Some(u),
            _ => None,
        }
    }
}

impl TypeYaml for TypeDef {
    fn yaml_string(&self) -> String {
        match self {
            TypeDef::Struct(x) => format!("\"{}\"", x.name),
            TypeDef::Enum(x) => format!("\"{}\"", x.name),
            TypeDef::Union(x) => format!("\"{}\"", x.name),
        }
    }
}

/// Definition of a struct
#[derive(Debug, Clone, PartialEq)]
pub struct StructDef {
    /// The name of the struct
    pub name: String,
    /// The vtable of the struct (name, type)
    pub vtable: Vec<(String, String)>,
    /// The size of the struct in bytes
    pub size: usize,
    /// The alignment of the struct in bytes
    pub alignment: usize,
    /// The members of the struct
    pub members: Vec<MemberDef>,
}
impl TypeYaml for StructDef {
    fn yaml_string(&self) -> String {
        let mut s = format!("  - name: '{}'\n", self.name);
        s.push_str(&format!("    size: 0x{:x}\n", self.size));
        s.push_str(&format!("    align: 0x{:x}\n", self.alignment));
        if !self.vtable.is_empty() {
            s.push_str("    vtable:\n");
            for (func, ty) in &self.vtable {
                s.push_str(&format!(
                    "      - {{ name: '{}', type: [ {} ] }}\n",
                    func, ty
                ));
            }
        }
        if !self.members.is_empty() {
            s.push_str("    members:\n");
            for member in &self.members {
                s.push_str(&member.yaml_string());
                s.push('\n');
            }
        }
        s
    }
}

/// Definition of a member of a struct
#[derive(Debug, Clone, PartialEq)]
pub struct MemberDef {
    /// Offset of the member in the struct
    pub offset: usize,
    /// Name of the member
    pub name: String,
    /// If the member is a base type
    pub is_base: bool,
    /// The type name of the member in YAML
    pub ty_yaml: String,
}

impl TypeYaml for MemberDef {
    fn yaml_string(&self) -> String {
        let mut s = format!("      '0x{:x}': {{ ", self.offset);
        if self.is_base {
            s.push_str("base: true, ");
        }
        s.push_str(&format!("name: '{}', ", self.name));
        s.push_str(&format!("type: [ {} ] }}", self.ty_yaml));
        s
    }
}

/// Definition of an enum
#[derive(Debug, Clone, PartialEq)]
pub struct EnumDef {
    /// The name of the enum
    pub name: String,
    /// The size of the enum in bytes
    pub size: usize,
    /// The enumerators of the enum (name, value)
    pub enumerators: Vec<(String, i128)>,
}
impl TypeYaml for EnumDef {
    fn yaml_string(&self) -> String {
        let mut s = format!("  - name: '{}'\n", self.name);
        s.push_str(&format!("    size: 0x{:x}\n", self.size));
        if !self.enumerators.is_empty() {
            s.push_str("    enumerators:\n");
            for (name, val) in &self.enumerators {
                if *val >= 0 {
                    s.push_str(&format!("      {name}: 0x{val:x}\n"))
                } else {
                    s.push_str(&format!("      {name}: {val}\n"))
                }
            }
        }
        s
    }
}

/// Definition of a union
#[derive(Debug, Clone, PartialEq)]
pub struct UnionDef {
    /// The name of the union
    pub name: String,
    /// The size of the union in bytes
    pub size: usize,
    /// The alignment of the union in bytes
    pub alignment: usize,
    /// The members of the union (name, type)
    pub members: Vec<(String, String)>,
}
impl TypeYaml for UnionDef {
    fn yaml_string(&self) -> String {
        let mut s = format!("  - name: '{}'\n", self.name);
        s.push_str(&format!("    size: 0x{:x}\n", self.size));
        s.push_str(&format!("    align: 0x{:x}\n", self.alignment));
        if !self.members.is_empty() {
            s.push_str("    members:\n");
            for (name, ty) in &self.members {
                s.push_str(&format!("      {name}: [ {ty} ]\n"))
            }
        }
        s
    }
}
