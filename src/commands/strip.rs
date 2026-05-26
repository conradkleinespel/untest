use crate::StripArgs;
use std::fs;
use std::path::Path;
use syn::Item;
use syn::spanned::Spanned;
use walkdir::WalkDir;

pub fn run(args: StripArgs) -> Result<(), Box<dyn std::error::Error>> {
    let input_dir = args.input;
    let output_dir = args.output;
    let excludes = args.exclude;

    println!("Stripping tests from {:?} to {:?}", input_dir, output_dir);

    fs::create_dir_all(&output_dir)?;

    for entry in WalkDir::new(&input_dir)
        .into_iter()
        .filter_entry(|e| should_include_entry(e.path(), &input_dir, &excludes))
    {
        let entry = entry?;
        let path = entry.path();
        println!("Entry {:?}", entry);

        process_entry(path, &input_dir, &output_dir)?;
    }

    println!("Test stripping complete");
    Ok(())
}

fn process_entry(
    path: &Path,
    input_dir: &Path,
    output_dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let relative_path = path.strip_prefix(input_dir)?;
    let output_path = output_dir.join(relative_path);
    println!("Output path will be {:?}", output_path);

    if path.is_dir() {
        fs::create_dir_all(&output_path)?;
    } else {
        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if path.extension().map_or(false, |ext| ext == "rs") {
            let content = fs::read_to_string(path)?;
            let stripped = strip_test_file(&content)?;
            fs::write(&output_path, stripped)?;
        } else {
            fs::copy(path, &output_path)?;
        }
    }
    Ok(())
}

fn should_include_entry(entry: &Path, input_dir: &Path, excludes: &[String]) -> bool {
    if entry
        .components()
        .any(|c| c.as_os_str().to_string_lossy() == ".git")
    {
        return false;
    }

    let relative_path = entry.strip_prefix(input_dir).unwrap_or(entry);
    let relative_path_str = relative_path.to_string_lossy();
    let path_str = entry.to_string_lossy();

    for exclude in excludes {
        if exclude.starts_with('/') {
            // Absolute-style path: check if it matches the start of the relative path
            let pattern = &exclude[1..];
            if relative_path_str == pattern
                || relative_path_str.starts_with(&format!("{}/", pattern))
            {
                return false;
            }
        } else {
            // Relative-style path: check if it's contained anywhere in the path
            if path_str.contains(exclude) {
                return false;
            }
        }
    }

    true
}

fn strip_test_file(content: &str) -> Result<String, Box<dyn std::error::Error>> {
    let file = syn::parse_file(content)?;
    let lines_to_remove = get_lines_to_remove(&file);
    Ok(apply_line_removals(content, &lines_to_remove))
}

fn get_lines_to_remove(file: &syn::File) -> Vec<(usize, usize)> {
    let mut lines_to_remove: Vec<(usize, usize)> = Vec::new();

    for item in &file.items {
        if let Some(attrs) = get_item_attrs(item) {
            if has_cfg_test(attrs) {
                // Get the span of this item
                let start = item.span().start();
                let end = item.span().end();
                lines_to_remove.push((start.line, end.line));
            }
        }
    }
    lines_to_remove
}

fn apply_line_removals(content: &str, lines_to_remove: &[(usize, usize)]) -> String {
    let mut result_lines: Vec<String> = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut skip_until_line: Option<usize> = None;

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1; // 1-indexed like syn

        // Check if we're in a skip range
        if let Some(skip_until) = skip_until_line {
            if line_num <= skip_until {
                continue;
            }
            skip_until_line = None;
        }

        // Check if this line starts a removal range
        for (start, end) in lines_to_remove {
            if line_num == *start {
                // Also look backwards for attribute lines
                let mut first_attr_line = line_num;
                for i in (0..idx).rev() {
                    let prev_line = lines[i].trim();
                    if prev_line.starts_with("#[") || prev_line.starts_with("#![") {
                        first_attr_line = i + 1;
                    } else if prev_line.is_empty() {
                        continue;
                    } else {
                        break;
                    }
                }

                // Remove lines from result_lines if they were already added
                while let Some(last_line_num) = result_lines.len().checked_add(0) {
                    if last_line_num >= first_attr_line {
                        result_lines.pop();
                    } else {
                        break;
                    }
                }

                skip_until_line = Some(*end);
                break;
            }
        }

        if skip_until_line.is_none() {
            result_lines.push(line.to_string());
        }
    }

    // Remove any starting newlines, this can happen if the file started with test code
    loop {
        if let Some(line) = result_lines.first()
            && line.is_empty()
        {
            result_lines = result_lines.split_off(1);
        } else {
            break;
        }
    }

    // Add ending newline back if it was in the original content
    let mut result = result_lines.join("\n");
    if content.ends_with('\n') && !result.ends_with('\n') {
        result.push('\n');
    }

    // Clean up excessive blank lines
    while result.contains("\n\n\n\n") {
        result = result.replace("\n\n\n\n", "\n\n");
    }

    result
}

fn get_item_attrs(item: &Item) -> Option<&[syn::Attribute]> {
    Some(match item {
        Item::Const(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::ExternCrate(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::ForeignMod(i) => &i.attrs,
        Item::Impl(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Struct(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::TraitAlias(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        Item::Verbatim(_) => return None,
        _ => return None,
    })
}

fn has_cfg_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        if attr.path().is_ident("test") {
            return true;
        }

        if attr.path().is_ident("cfg") {
            if let Ok(meta) = attr.parse_args::<syn::Expr>() {
                if expr_contains_test(&meta) {
                    return true;
                }
            }
        }

        if attr.path().is_ident("cfg_attr") {
            let mut is_test = false;
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("test") {
                    is_test = true;
                }
                Ok(())
            });

            // Fallback for simple string check if parse_nested_meta didn't find it
            if !is_test {
                use quote::ToTokens;
                let tokens = attr.to_token_stream().to_string();
                if tokens.contains("test") {
                    is_test = true;
                }
            }

            if is_test {
                return true;
            }
        }

        false
    })
}

fn expr_contains_test(expr: &syn::Expr) -> bool {
    match expr {
        syn::Expr::Paren(paren) => expr_contains_test(&paren.expr),
        syn::Expr::Path(path) => {
            if let Some(ident) = path.path.get_ident() {
                ident == "test"
            } else {
                false
            }
        }
        syn::Expr::Binary(binary) => {
            expr_contains_test(&binary.left) || expr_contains_test(&binary.right)
        }
        syn::Expr::Call(call) => {
            if let syn::Expr::Path(path) = &*call.func {
                if let Some(ident) = path.path.get_ident() {
                    if ident == "all" || ident == "any" || ident == "not" {
                        return call.args.iter().any(expr_contains_test);
                    }
                }
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_simple_test_fn() {
        let input = r#"fn production_code() {}

#[test]
fn test_something() {
    assert!(true);
}
"#;
        let output = strip_test_file(input).unwrap();
        assert_eq!(output, "fn production_code() {}\n");
    }

    #[test]
    fn test_strip_cfg_test_mod() {
        let input = r#"fn production_code() {}

#[cfg(test)]
mod tests {
    #[test]
    fn test_foo() {}
}
"#;
        let output = strip_test_file(input).unwrap();
        assert_eq!(output, "fn production_code() {}\n");
    }

    #[test]
    fn test_preserve_production_code() {
        let input = r#"fn main() {
    println!("Hello, world!");
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

fn another_func() {}
"#;
        let output = strip_test_file(input).unwrap();
        assert_eq!(
            output,
            "fn main() {\n    println!(\"Hello, world!\");\n}\n\n\nfn another_func() {}\n"
        );
    }

    #[test]
    fn test_should_include_entry() {
        let input_dir = Path::new("/base");
        let excludes = vec![
            "/target".to_string(),
            "/target/my/long/path.txt".to_string(),
            "node_modules".to_string(),
        ];

        // .git should always be excluded
        assert!(!should_include_entry(
            Path::new("/base/.git"),
            input_dir,
            &excludes
        ));
        assert!(!should_include_entry(
            Path::new("/base/src/.git"),
            input_dir,
            &excludes
        ));

        // Absolute-style excludes
        assert!(!should_include_entry(
            Path::new("/base/target"),
            input_dir,
            &excludes
        ));
        assert!(!should_include_entry(
            Path::new("/base/target/file.txt"),
            input_dir,
            &excludes
        ));
        assert!(!should_include_entry(
            Path::new("/base/target/my/long/path.txt"),
            input_dir,
            &excludes
        ));
        assert!(should_include_entry(
            Path::new("/base/src/target"),
            input_dir,
            &excludes
        ));

        // Relative-style excludes
        assert!(!should_include_entry(
            Path::new("/base/node_modules"),
            input_dir,
            &excludes
        ));
        assert!(!should_include_entry(
            Path::new("/base/src/node_modules"),
            input_dir,
            &excludes
        ));

        // Normal files
        assert!(should_include_entry(
            Path::new("/base/src/main.rs"),
            input_dir,
            &excludes
        ));
    }

    #[test]
    fn test_apply_line_removals() {
        let content = "line1\nline2\n#[test]\nfn test() {\n}\nline6";
        let lines_to_remove = vec![(4, 5)];
        let result = apply_line_removals(content, &lines_to_remove);
        assert_eq!(result, "line1\nline2\nline6");
    }

    #[test]
    fn test_expr_contains_test() {
        let expr: syn::Expr = syn::parse_str("test").unwrap();
        assert!(expr_contains_test(&expr));

        let expr: syn::Expr = syn::parse_str("(test)").unwrap();
        assert!(expr_contains_test(&expr));

        let expr: syn::Expr = syn::parse_str("all(unix, test)").unwrap();
        assert!(expr_contains_test(&expr));

        let expr: syn::Expr = syn::parse_str("any(windows, test)").unwrap();
        assert!(expr_contains_test(&expr));

        let expr: syn::Expr = syn::parse_str("not(test)").unwrap();
        assert!(expr_contains_test(&expr));

        let expr: syn::Expr = syn::parse_str("debug_assertions").unwrap();
        assert!(!expr_contains_test(&expr));
    }

    #[test]
    fn test_has_cfg_test() {
        let attr: syn::Attribute = syn::parse_quote!(#[test]);
        assert!(has_cfg_test(&[attr]));

        let attr: syn::Attribute = syn::parse_quote!(#[cfg(test)]);
        assert!(has_cfg_test(&[attr]));

        let attr: syn::Attribute = syn::parse_quote!(#[cfg(all(feature = "foo", test))]);
        assert!(has_cfg_test(&[attr]));

        let attr: syn::Attribute = syn::parse_quote!(#[cfg_attr(feature = "bar", test)]);
        assert!(has_cfg_test(&[attr]));

        let attr: syn::Attribute = syn::parse_quote!(#[derive(Debug)]);
        assert!(!has_cfg_test(&[attr]));
    }

    #[test]
    fn test_strip_cfg_attr() {
        let input = r#"#[cfg_attr(feature = "magic", test)]
fn magic_test() {}

fn prod() {}
"#;
        let output = strip_test_file(input).unwrap();
        assert_eq!(output, "fn prod() {}\n");
    }

    #[test]
    fn test_get_item_attrs() {
        let item: syn::Item = syn::parse_quote!(
            #[must_use]
            fn foo() {}
        );
        let attrs = get_item_attrs(&item).unwrap();
        assert_eq!(attrs.len(), 1);
        assert!(attrs[0].path().is_ident("must_use"));

        let item: syn::Item = syn::parse_quote!(
            #[derive(Debug)]
            struct Bar;
        );
        let attrs = get_item_attrs(&item).unwrap();
        assert_eq!(attrs.len(), 1);
        assert!(attrs[0].path().is_ident("derive"));
    }

    #[test]
    fn test_process_entry_file() {
        let temp = std::env::temp_dir().join("untest_test_file");
        let input_dir = temp.join("input");
        let output_dir = temp.join("output");
        fs::create_dir_all(&input_dir).unwrap();

        let file_path = input_dir.join("lib.rs");
        let content = "fn prod() {}\n#[test]\nfn t() {}";
        fs::write(&file_path, content).unwrap();

        process_entry(&file_path, &input_dir, &output_dir).unwrap();

        let output_path = output_dir.join("lib.rs");
        assert!(output_path.exists());
        let output_content = fs::read_to_string(output_path).unwrap();
        assert_eq!(output_content, "fn prod() {}");

        fs::remove_dir_all(temp).unwrap();
    }

    #[test]
    fn test_process_entry_copy_non_rs() {
        let temp = std::env::temp_dir().join("untest_test_copy");
        let input_dir = temp.join("input");
        let output_dir = temp.join("output");
        fs::create_dir_all(&input_dir).unwrap();

        let file_path = input_dir.join("README.md");
        let content = "Just a readme";
        fs::write(&file_path, content).unwrap();

        process_entry(&file_path, &input_dir, &output_dir).unwrap();

        let output_path = output_dir.join("README.md");
        assert!(output_path.exists());
        assert_eq!(fs::read_to_string(output_path).unwrap(), content);

        fs::remove_dir_all(temp).unwrap();
    }
}
