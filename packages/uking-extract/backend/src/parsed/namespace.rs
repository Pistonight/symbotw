use std::borrow::Cow;
use std::collections::BTreeMap;

use error_stack::{Result, ResultExt};

use super::TypeError;

/// Namespace info
#[derive(Debug, Default, Clone)]
pub struct Namespace<'a>(Vec<Cow<'a, str>>);
impl<'a> Namespace<'a> {
    pub fn push(&mut self, name: impl Into<Cow<'a, str>>) {
        self.0.push(name.into());
    }

    pub fn pop(&mut self) {
        if self.0.pop().is_none() {
            panic!("Namespace::pop() called on empty stack");
        }
    }

    /// Get the name prefixed by this namespace
    pub fn get_with(&self, name: &str) -> String {
        let mut stack = self.0.clone();
        stack.push(Cow::Borrowed(name));
        stack.join("::")
    }
}

#[derive(Debug)]
pub struct NamespaceMap<'a> {
    map: BTreeMap<usize, Namespace<'a>>,
}

impl<'a> From<BTreeMap<usize, Namespace<'a>>> for NamespaceMap<'a> {
    fn from(map: BTreeMap<usize, Namespace<'a>>) -> Self {
        Self { map }
    }
}

impl<'a> AsRef<BTreeMap<usize, Namespace<'a>>> for NamespaceMap<'a> {
    fn as_ref(&self) -> &BTreeMap<usize, Namespace<'a>> {
        &self.map
    }
}

impl<'a> NamespaceMap<'a> {
    /// Get the name prefixed by the namespace at the given offset
    pub fn get(&self, offset: usize, name: &str) -> Result<String, TypeError> {
        let ns = self
            .map
            .get(&offset)
            .ok_or(TypeError::UnlinkedNamespace(offset))
            .attach_printable_lazy(|| format!("While getting for name: {}", name))?;
        Ok(ns.get_with(name))
    }
}
