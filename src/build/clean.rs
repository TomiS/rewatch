use super::build_types::*;
use super::packages;
use crate::bsconfig;
use crate::helpers;
use crate::helpers::emojis::*;
use ahash::AHashSet;
use console::style;
use rayon::prelude::*;
use std::io::Write;
use std::time::Instant;

fn remove_ast(source_file: &str, package_name: &str, root_path: &str, is_root: bool) {
    let _ = std::fs::remove_file(helpers::get_compiler_asset(
        source_file,
        package_name,
        &packages::Namespace::NoNamespace,
        root_path,
        "ast",
        is_root,
    ));
}

fn remove_iast(source_file: &str, package_name: &str, root_path: &str, is_root: bool) {
    let _ = std::fs::remove_file(helpers::get_compiler_asset(
        source_file,
        package_name,
        &packages::Namespace::NoNamespace,
        root_path,
        "iast",
        is_root,
    ));
}

fn remove_mjs_file(source_file: &str, suffix: &bsconfig::Suffix) {
    let _ = std::fs::remove_file(helpers::change_extension(
        source_file,
        // suffix.to_string includes the ., so we need to remove it
        &suffix.to_string()[1..],
    ));
}

fn remove_compile_asset(
    source_file: &str,
    package_name: &str,
    namespace: &packages::Namespace,
    root_path: &str,
    is_root: bool,
    extension: &str,
) {
    let _ = std::fs::remove_file(helpers::get_compiler_asset(
        source_file,
        package_name,
        namespace,
        root_path,
        extension,
        is_root,
    ));
    let _ = std::fs::remove_file(helpers::get_bs_compiler_asset(
        source_file,
        package_name,
        namespace,
        root_path,
        extension,
        is_root,
    ));
}

pub fn remove_compile_assets(
    source_file: &str,
    package_name: &str,
    namespace: &packages::Namespace,
    root_path: &str,
    is_root: bool,
) {
    // optimization
    // only issue cmti if htere is an interfacce file
    for extension in &["cmj", "cmi", "cmt", "cmti"] {
        remove_compile_asset(
            source_file,
            package_name,
            namespace,
            root_path,
            is_root,
            extension,
        );
    }
}

pub fn clean_mjs_files(build_state: &BuildState, project_root: &str) {
    // get all rescript file locations
    let rescript_file_locations = build_state
        .modules
        .values()
        .filter_map(|module| match &module.source_type {
            SourceType::SourceFile(source_file) => {
                let package = build_state.packages.get(&module.package_name).unwrap();
                let root_package = build_state
                    .packages
                    .get(&build_state.root_config_name)
                    .expect("Could not find root package");
                Some((
                    std::path::PathBuf::from(helpers::get_package_path(
                        &project_root,
                        &module.package_name,
                        package.is_root,
                    ))
                    .join(source_file.implementation.path.to_string())
                    .to_string_lossy()
                    .to_string(),
                    root_package
                        .bsconfig
                        .suffix
                        .to_owned()
                        .unwrap_or(bsconfig::Suffix::Mjs),
                ))
            }
            _ => None,
        })
        .collect::<Vec<(String, bsconfig::Suffix)>>();

    rescript_file_locations
        .par_iter()
        .for_each(|(rescript_file_location, suffix)| remove_mjs_file(&rescript_file_location, &suffix));
}

// TODO: change to scan_previous_build => CompileAssetsState
// and then do cleanup on that state (for instance remove all .mjs files that are not in the state)

pub fn cleanup_previous_build(
    build_state: &mut BuildState,
    compile_assets_state: CompileAssetsState,
) -> (usize, usize, AHashSet<String>) {
    // delete the .mjs file which appear in our previous compile assets
    // but does not exists anymore
    // delete the compiler assets for which modules we can't find a rescript file
    // location of rescript file is in the AST
    // delete the .mjs file for which we DO have a compiler asset, but don't have a
    // rescript file anymore (path is found in the .ast file)
    let diff = compile_assets_state
        .ast_rescript_file_locations
        .difference(&compile_assets_state.rescript_file_locations)
        .collect::<Vec<&String>>();

    let diff_len = diff.len();

    let deleted_interfaces = diff
        .par_iter()
        .map(|res_file_location| {
            let AstModule {
                module_name,
                package_name,
                namespace: package_namespace,
                ast_file_path,
                is_root,
                suffix,
                ..
            } = compile_assets_state
                .ast_modules
                .get(&res_file_location.to_string())
                .expect("Could not find module name for ast file");
            remove_compile_assets(
                res_file_location,
                package_name,
                package_namespace,
                &build_state.project_root,
                *is_root,
            );
            remove_mjs_file(
                &res_file_location,
                &suffix.to_owned().unwrap_or(bsconfig::Suffix::Mjs),
            );
            remove_iast(
                res_file_location,
                package_name,
                &build_state.project_root,
                *is_root,
            );
            remove_ast(
                res_file_location,
                package_name,
                &build_state.project_root,
                *is_root,
            );
            match helpers::get_extension(ast_file_path).as_str() {
                "iast" => Some(module_name.to_owned()),
                "ast" => None,
                _ => None,
            }
        })
        .collect::<Vec<Option<String>>>()
        .iter()
        .filter_map(|module_name| module_name.to_owned())
        .collect::<AHashSet<String>>();

    compile_assets_state
        .ast_rescript_file_locations
        .intersection(&compile_assets_state.rescript_file_locations)
        .into_iter()
        .for_each(|res_file_location| {
            let AstModule {
                module_name,
                last_modified: ast_last_modified,
                ast_file_path,
                ..
            } = compile_assets_state
                .ast_modules
                .get(res_file_location)
                .expect("Could not find module name for ast file");
            let module = build_state
                .modules
                .get_mut(module_name)
                .expect("Could not find module for ast file");

            let compile_dirty = compile_assets_state.cmi_modules.get(module_name);
            if let Some(compile_dirty) = compile_dirty {
                let last_modified = Some(ast_last_modified);

                if let Some(last_modified) = last_modified {
                    if compile_dirty > &last_modified && !deleted_interfaces.contains(module_name) {
                        module.compile_dirty = false;
                    }
                }
            }

            match &mut module.source_type {
                SourceType::MlMap(_) => unreachable!("MlMap is not matched with a ReScript file"),
                SourceType::SourceFile(source_file) => {
                    if helpers::is_interface_ast_file(ast_file_path) {
                        let interface = source_file
                            .interface
                            .as_mut()
                            .expect("Could not find interface for module");

                        let source_last_modified = interface.last_modified;
                        if ast_last_modified > &source_last_modified {
                            interface.dirty = false;
                        }
                    } else {
                        let implementation = &mut source_file.implementation;
                        let source_last_modified = implementation.last_modified;
                        if ast_last_modified > &source_last_modified
                            && !deleted_interfaces.contains(module_name)
                        {
                            implementation.dirty = false;
                        }
                    }
                }
            }
        });

    compile_assets_state
        .cmi_modules
        .iter()
        .for_each(|(module_name, last_modified)| {
            build_state.modules.get_mut(module_name).map(|module| {
                module.last_compiled_cmi = Some(*last_modified);
            });
        });

    compile_assets_state
        .cmt_modules
        .iter()
        .for_each(|(module_name, last_modified)| {
            build_state.modules.get_mut(module_name).map(|module| {
                module.last_compiled_cmt = Some(*last_modified);
            });
        });

    let ast_module_names = compile_assets_state
        .ast_modules
        .values()
        .filter_map(
            |AstModule {
                 module_name,
                 ast_file_path,
                 ..
             }| {
                match helpers::get_extension(ast_file_path).as_str() {
                    "iast" => None,
                    "ast" => Some(module_name),
                    _ => None,
                }
            },
        )
        .collect::<AHashSet<&String>>();

    let all_module_names = build_state
        .modules
        .keys()
        .map(|module_name| module_name)
        .collect::<AHashSet<&String>>();

    let deleted_module_names = ast_module_names
        .difference(&all_module_names)
        .map(|module_name| {
            // if the module is a namespace, we need to mark the whole namespace as dirty when a module has been deleted
            if let Some(namespace) = helpers::get_namespace_from_module_name(module_name) {
                return namespace;
            }
            return module_name.to_string();
        })
        .collect::<AHashSet<String>>();

    (
        diff_len,
        compile_assets_state.ast_rescript_file_locations.len(),
        deleted_module_names,
    )
}

fn failed_to_parse(module: &Module) -> bool {
    match &module.source_type {
        SourceType::SourceFile(SourceFile {
            implementation:
                Implementation {
                    parse_state: ParseState::ParseError | ParseState::Warning,
                    ..
                },
            ..
        }) => true,
        SourceType::SourceFile(SourceFile {
            interface:
                Some(Interface {
                    parse_state: ParseState::ParseError | ParseState::Warning,
                    ..
                }),
            ..
        }) => true,
        _ => false,
    }
}

fn failed_to_compile(module: &Module) -> bool {
    match &module.source_type {
        SourceType::SourceFile(SourceFile {
            implementation:
                Implementation {
                    compile_state: CompileState::Error | CompileState::Warning,
                    ..
                },
            ..
        }) => true,
        SourceType::SourceFile(SourceFile {
            interface:
                Some(Interface {
                    compile_state: CompileState::Error | CompileState::Warning,
                    ..
                }),
            ..
        }) => true,
        _ => false,
    }
}

pub fn cleanup_after_build(build_state: &BuildState) {
    build_state.modules.par_iter().for_each(|(_module_name, module)| {
        let package = build_state.get_package(&module.package_name).unwrap();
        if failed_to_parse(module) {
            match &module.source_type {
                SourceType::SourceFile(source_file) => {
                    remove_iast(
                        &source_file.implementation.path,
                        &module.package_name,
                        &build_state.project_root,
                        package.is_root,
                    );
                    remove_ast(
                        &source_file.implementation.path,
                        &module.package_name,
                        &build_state.project_root,
                        package.is_root,
                    );
                }
                _ => (),
            }
        }
        if failed_to_compile(module) {
            // only retain ast file if it compiled successfully, that's the only thing we check
            // if we see a AST file, we assume it compiled successfully, so we also need to clean
            // up the AST file if compile is not successful
            match &module.source_type {
                SourceType::SourceFile(source_file) => {
                    // we only clean the cmt (typed tree) here, this will cause the file to be recompiled
                    // (and thus keep showing the warning), but it will keep the cmi file, so that we don't
                    // unecessary mark all the dependents as dirty, when there is no change in the interface
                    remove_compile_asset(
                        &source_file.implementation.path,
                        &module.package_name,
                        &package.namespace,
                        &build_state.project_root,
                        package.is_root,
                        "cmt",
                    );
                }
                SourceType::MlMap(_) => (),
            }
        }
    });
}

pub fn clean(path: &str) {
    let project_root = helpers::get_abs_path(path);
    let packages = packages::make(&None, &project_root);
    let root_config_name = packages::get_package_name(&project_root);

    let timing_clean_compiler_assets = Instant::now();
    print!(
        "{} {} Cleaning compiler assets...",
        style("[1/2]").bold().dim(),
        SWEEP
    );
    std::io::stdout().flush().unwrap();
    packages.iter().for_each(|(_, package)| {
        print!(
            "{}\r{} {} Cleaning {}...",
            LINE_CLEAR,
            style("[1/2]").bold().dim(),
            SWEEP,
            package.name
        );
        std::io::stdout().flush().unwrap();

        let path_str = helpers::get_build_path(&project_root, &package.name, package.is_root);
        let path = std::path::Path::new(&path_str);
        let _ = std::fs::remove_dir_all(path);

        let path_str = helpers::get_bs_build_path(&project_root, &package.name, package.is_root);
        let path = std::path::Path::new(&path_str);
        let _ = std::fs::remove_dir_all(path);
    });
    let timing_clean_compiler_assets_elapsed = timing_clean_compiler_assets.elapsed();

    println!(
        "{}\r{} {}Cleaned compiler assets in {:.2}s",
        LINE_CLEAR,
        style("[1/2]").bold().dim(),
        CHECKMARK,
        timing_clean_compiler_assets_elapsed.as_secs_f64()
    );
    std::io::stdout().flush().unwrap();

    let timing_clean_mjs = Instant::now();
    print!("{} {} Cleaning mjs files...", style("[2/2]").bold().dim(), SWEEP);
    std::io::stdout().flush().unwrap();
    let mut build_state = BuildState::new(project_root.to_owned(), root_config_name, packages);
    packages::parse_packages(&mut build_state);
    clean_mjs_files(&build_state, &project_root);
    let timing_clean_mjs_elapsed = timing_clean_mjs.elapsed();
    println!(
        "{}\r{} {}Cleaned mjs files in {:.2}s",
        LINE_CLEAR,
        style("[2/2]").bold().dim(),
        CHECKMARK,
        timing_clean_mjs_elapsed.as_secs_f64()
    );
    std::io::stdout().flush().unwrap();
}
