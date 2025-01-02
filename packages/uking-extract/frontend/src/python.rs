use std::collections::{HashMap, HashSet};

use anyhow::bail;

use crate::Frontend;

pub struct Bundler {
    /// Builtin libraries (from /python)
    builtin_libs: HashMap<String, &'static str>,
    /// Library identifiers that are already imported in
    imported_libs: HashSet<String>,

    main_imports: Vec<&'static str>,

    /// Header script
    header: String,
}

macro_rules! import_builtin {
    ($libs: ident, $path: literal) => {
        $libs.insert(
            $path.to_string(),
            include_str!(concat!("../python/", $path, ".py")),
        )
    };
}

impl Bundler {
    pub fn new() -> Self {
        let mut libs = HashMap::new();
        import_builtin!(libs, "addr_importer");
        import_builtin!(libs, "addrdef");
        import_builtin!(libs, "common");
        import_builtin!(libs, "enumdef");
        import_builtin!(libs, "frontend");
        import_builtin!(libs, "heuristics");
        import_builtin!(libs, "memberdef");
        import_builtin!(libs, "printutil");
        import_builtin!(libs, "run");
        import_builtin!(libs, "structdef");
        import_builtin!(libs, "type_importer");
        import_builtin!(libs, "tyyaml");
        import_builtin!(libs, "uniondef");

        Self {
            builtin_libs: libs,
            imported_libs: HashSet::new(),
            header: String::new(),
            main_imports: vec![
                "enumdef",
                "structdef",
                "uniondef",
                "memberdef",
                "json",
                "run",
            ],
        }
    }

    /// Import the frontend script
    pub fn import_frontend(&mut self, frontend: Frontend) {
        let libs = &mut self.builtin_libs;
        match frontend {
            Frontend::IDA => {
                import_builtin!(libs, "frontend_impl_ida");
                self.main_imports.push("frontend_impl_ida")
            }
        };
    }

    /// Set the header before the script, wrapped in a comment block
    pub fn set_header(&mut self, header: String) {
        self.header = header;
    }

    /// Bundle the main script.
    pub fn bundle(mut self, main_script: &str) -> anyhow::Result<String> {
        let mut out_imports = String::new();
        let mut out_script = String::new();
        let mut stack = Vec::new();

        let import_script = self
            .main_imports
            .iter()
            .map(|x| format!("import {x}\n"))
            .collect::<Vec<_>>()
            .join("");

        self.process_script_recur(
            &import_script,
            &mut stack,
            &mut out_imports,
            &mut out_script,
        )?;
        self.process_script_recur(main_script, &mut stack, &mut out_imports, &mut out_script)?;

        let mut out = String::new();
        out.push_str("\"\"\"\n");
        out.push_str(&self.header);
        out.push_str("\n\"\"\"\n");
        out.push_str(&out_imports);
        out.push('\n');
        out.push_str(&out_script);

        Ok(out)
    }

    fn process_script_recur(
        &mut self,
        script: &str,
        import_stack: &mut Vec<String>,
        out_imports: &mut String,
        out_script: &mut String,
    ) -> anyhow::Result<()> {
        let mut current_script = String::new();

        for line in script.lines() {
            let to_import = get_import_lib_name_from_line(line);
            match to_import {
                None => {
                    // not an import
                    current_script.push_str(line);
                    current_script.push('\n');
                }
                Some(lib) => {
                    if !self.imported_libs.insert(lib.to_string()) {
                        continue; // already imported
                    }
                    if import_stack.iter().any(|x| x == lib) {
                        bail!("circular import: {:?} -> {}", import_stack, lib)
                    }
                    match self.builtin_libs.get(lib) {
                        None => {
                            // python lib, just write the import statement
                            out_imports.push_str(line);
                            out_imports.push('\n');
                        }
                        Some(lib_content) => {
                            // bundle in the script content
                            import_stack.push(lib.to_string());
                            self.process_script_recur(
                                lib_content,
                                import_stack,
                                out_imports,
                                out_script,
                            )?;
                            import_stack.pop();
                        }
                    }
                }
            }
        }

        out_script.push_str(&current_script);
        Ok(())
    }
}
fn get_import_lib_name_from_line(line: &str) -> Option<&str> {
    if let Some(lib) = line.strip_prefix("import ") {
        let lib = lib.trim_end();
        return Some(lib);
    }
    if let Some(lib) = line.strip_prefix("from ") {
        let mut parts = lib.split_whitespace();
        if let Some(lib) = parts.next() {
            if let Some(i) = parts.next() {
                if i == "import" {
                    return Some(lib);
                }
            }
        }
    }
    None
}
