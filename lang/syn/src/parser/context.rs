use {
    anyhow::{anyhow, Result},
    std::{
        collections::BTreeMap,
        path::{Path, PathBuf},
    },
    syn::{
        parse::{Error as ParseError, Result as ParseResult},
        Ident, ImplItem, ImplItemConst, Type, TypePath,
    },
};

/// Crate parse context
///
/// Keeps track of modules defined within a crate.
pub struct CrateContext {
    modules: BTreeMap<String, ParsedModule>,
}

impl CrateContext {
    pub fn parse(root: impl AsRef<Path>) -> Result<Self> {
        Ok(CrateContext {
            modules: ParsedModule::parse_recursive(root.as_ref())?,
        })
    }

    pub fn consts(&self) -> impl Iterator<Item = &syn::ItemConst> {
        self.modules.values().flat_map(|ctx| ctx.consts())
    }

    pub fn impl_consts(&self) -> impl Iterator<Item = (&Ident, &syn::ImplItemConst)> {
        self.modules.values().flat_map(|ctx| ctx.impl_consts())
    }

    pub fn structs(&self) -> impl Iterator<Item = &syn::ItemStruct> {
        self.modules.values().flat_map(|ctx| ctx.structs())
    }

    pub fn enums(&self) -> impl Iterator<Item = &syn::ItemEnum> {
        self.modules.values().flat_map(|ctx| ctx.enums())
    }

    pub fn type_aliases(&self) -> impl Iterator<Item = &syn::ItemType> {
        self.modules.values().flat_map(|ctx| ctx.type_aliases())
    }

    pub fn modules(&self) -> impl Iterator<Item = ModuleContext<'_>> {
        self.modules.values().map(|detail| ModuleContext { detail })
    }

    pub fn root_module(&self) -> ModuleContext<'_> {
        #[allow(
            clippy::unwrap_used,
            reason = "\"crate\" module is always inserted during parsing"
        )]
        let detail = self.modules.get("crate").unwrap();
        ModuleContext { detail }
    }

    // Perform Anchor safety checks on the parsed create
    pub fn safety_checks(&self) -> Result<()> {
        // Check all structs for unsafe field types, i.e. AccountInfo and UncheckedAccount.
        for ctx in self.modules.values() {
            for unsafe_field in ctx.unsafe_struct_fields() {
                // Check if unsafe field type has been documented with a /// SAFETY: doc string.
                let is_documented = unsafe_field.attrs.iter().any(|attr| {
                    if let syn::Meta::NameValue(syn::MetaNameValue {
                        value:
                            syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Str(s),
                                ..
                            }),
                        ..
                    }) = &attr.meta
                    {
                        s.value().contains("CHECK")
                    } else {
                        false
                    }
                });
                if !is_documented {
                    #[allow(
                        clippy::unwrap_used,
                        reason = "unsafe fields always have idents (named fields only)"
                    )]
                    let ident = unsafe_field.ident.as_ref().unwrap();
                    let span = ident.span();
                    // Error if undocumented.
                    #[allow(
                        clippy::unwrap_used,
                        reason = "file paths are always valid during compilation"
                    )]
                    let canonical = ctx.file.canonicalize().unwrap();
                    return Err(anyhow!(
                        r#"
        {}:{}:{}
        Struct field "{}" is unsafe, but is not documented.
        Please add a `/// CHECK:` doc comment explaining why no checks through types are necessary.
        Alternatively, for reasons like quick prototyping, you may disable the safety checks
        by using the `skip-lint` option.
        See https://www.anchor-lang.com/docs/basics/program-structure#account-validation for more information.
                    "#,
                        canonical.display(),
                        span.start().line,
                        span.start().column,
                        ident
                    ));
                };
            }
        }
        Ok(())
    }
}

/// Module parse context
///
/// Keeps track of items defined within a module.
#[derive(Copy, Clone)]
pub struct ModuleContext<'krate> {
    detail: &'krate ParsedModule,
}

impl ModuleContext<'_> {
    pub fn items(&self) -> impl Iterator<Item = &syn::Item> {
        self.detail.items.iter()
    }
}
struct ParsedModule {
    name: String,
    file: PathBuf,
    path: String,
    items: Vec<syn::Item>,
}

struct UnparsedModule {
    file: PathBuf,
    path: String,
    name: String,
    item: syn::ItemMod,
}

impl ParsedModule {
    fn parse_recursive(root: &Path) -> Result<BTreeMap<String, ParsedModule>> {
        let mut modules = BTreeMap::new();

        let root_content = std::fs::read_to_string(root)?;
        let root_file = syn::parse_file(&root_content)?;
        let root_mod = Self::new(
            String::new(),
            root.to_owned(),
            "crate".to_owned(),
            root_file.items,
        );

        let mut unparsed = root_mod.unparsed_submodules();
        while let Some(to_parse) = unparsed.pop() {
            let path = format!("{}::{}", to_parse.path, to_parse.name);
            let module = Self::from_item_mod(&to_parse.file, &path, to_parse.item)?;

            unparsed.extend(module.unparsed_submodules());
            modules.insert(format!("{}{}", module.path, to_parse.name), module);
        }

        modules.insert(root_mod.name.clone(), root_mod);

        Ok(modules)
    }

    fn from_item_mod(
        parent_file: &Path,
        parent_path: &str,
        item: syn::ItemMod,
    ) -> ParseResult<Self> {
        Ok(match item.content {
            Some((_, items)) => {
                // The module content is within the parent file being parsed
                Self::new(
                    parent_path.to_owned(),
                    parent_file.to_owned(),
                    item.ident.to_string(),
                    items,
                )
            }
            None => {
                // The module is referencing some other file, so we need to load that
                // to parse the items it has.
                #[allow(
                    clippy::unwrap_used,
                    reason = "file paths always have parent directories during compilation"
                )]
                let parent_dir = parent_file.parent().unwrap();
                #[allow(
                    clippy::unwrap_used,
                    reason = "file stems are always valid UTF-8 Rust identifiers"
                )]
                let parent_filename = parent_file.file_stem().unwrap().to_str().unwrap();
                let parent_mod_dir = parent_dir.join(parent_filename);

                let possible_file_paths = vec![
                    parent_dir.join(format!("{}.rs", item.ident)),
                    parent_dir.join(format!("{}/mod.rs", item.ident)),
                    parent_mod_dir.join(format!("{}.rs", item.ident)),
                    parent_mod_dir.join(format!("{}/mod.rs", item.ident)),
                ];

                let mod_file_path = possible_file_paths
                    .into_iter()
                    .find(|p| p.exists())
                    .ok_or_else(|| ParseError::new_spanned(&item, "could not find file"))?;
                let mod_file_content = std::fs::read_to_string(&mod_file_path)
                    .map_err(|_| ParseError::new_spanned(&item, "could not read file"))?;
                let mod_file = syn::parse_file(&mod_file_content)?;

                Self::new(
                    parent_path.to_owned(),
                    mod_file_path,
                    item.ident.to_string(),
                    mod_file.items,
                )
            }
        })
    }

    fn new(path: String, file: PathBuf, name: String, items: Vec<syn::Item>) -> Self {
        Self {
            name,
            file,
            path,
            items,
        }
    }

    fn unparsed_submodules(&self) -> Vec<UnparsedModule> {
        self.submodules()
            .map(|item| UnparsedModule {
                file: self.file.clone(),
                path: self.path.clone(),
                name: item.ident.to_string(),
                item: item.clone(),
            })
            .collect()
    }

    fn submodules(&self) -> impl Iterator<Item = &syn::ItemMod> {
        self.items.iter().filter_map(|i| match i {
            syn::Item::Mod(item) => Some(item),
            _ => None,
        })
    }

    fn structs(&self) -> impl Iterator<Item = &syn::ItemStruct> {
        self.items.iter().filter_map(|i| match i {
            syn::Item::Struct(item) => Some(item),
            _ => None,
        })
    }

    fn unsafe_struct_fields(&self) -> impl Iterator<Item = &syn::Field> {
        let accounts_filter = |item_struct: &&syn::ItemStruct| {
            item_struct.attrs.iter().any(|attr| {
                attr.path().is_ident("derive")
                    && attr
                        .parse_args_with(
                            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
                        )
                        .ok()
                        .is_some_and(|args| {
                            args.iter()
                                .any(|m| matches!(m, syn::Meta::Path(p) if p.is_ident("Accounts")))
                        })
            })
        };

        self.structs()
            .filter(accounts_filter)
            .flat_map(|s| &s.fields)
            .filter(|f| match &f.ty {
                syn::Type::Path(syn::TypePath {
                    path: syn::Path { segments, .. },
                    ..
                }) => {
                    segments.len() == 1 && segments[0].ident == "UncheckedAccount"
                        || segments[0].ident == "AccountInfo"
                }
                _ => false,
            })
    }

    fn enums(&self) -> impl Iterator<Item = &syn::ItemEnum> {
        self.items.iter().filter_map(|i| match i {
            syn::Item::Enum(item) => Some(item),
            _ => None,
        })
    }

    fn type_aliases(&self) -> impl Iterator<Item = &syn::ItemType> {
        self.items.iter().filter_map(|i| match i {
            syn::Item::Type(item) => Some(item),
            _ => None,
        })
    }

    fn consts(&self) -> impl Iterator<Item = &syn::ItemConst> {
        self.items.iter().filter_map(|i| match i {
            syn::Item::Const(item) => Some(item),
            _ => None,
        })
    }

    fn impl_consts(&self) -> impl Iterator<Item = (&Ident, &ImplItemConst)> {
        self.items
            .iter()
            .filter_map(|i| match i {
                syn::Item::Impl(syn::ItemImpl {
                    self_ty: ty, items, ..
                }) => {
                    if let Type::Path(TypePath {
                        qself: None,
                        path: p,
                    }) = ty.as_ref()
                    {
                        if let Some(ident) = p.get_ident() {
                            let mut to_return = Vec::new();
                            items.iter().for_each(|item| {
                                if let ImplItem::Const(item) = item {
                                    to_return.push((ident, item));
                                }
                            });
                            Some(to_return)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .flatten()
    }
}
