#![allow(clippy::uninlined_format_args)]
//! Command-line interface

pub mod progress;

use crate::config::{CollectionCliArgs, CollectionConfigFile};
use crate::error::{CityJsonStacError, Result};
use crate::memory::{log_memory, memory_log_interval, memory_logging_enabled};
use crate::metadata::CRS;
use crate::reader::{get_reader_from_source, InputSource};
use crate::stac::{StacCollectionBuilder, StacItemBuilder};
use crate::traversal;
use clap::{Parser, Subcommand};
use progress::{
    create_progress_bar, create_spinner, finish_spinner_err, finish_spinner_ok, print_banner,
    print_error, print_info, print_success, print_warning, Summary,
};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "citystac")]
#[command(author, version, about = "Generate STAC metadata for CityJSON datasets", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Dry run: validate config and inputs without generating output
    #[arg(long, global = true)]
    dry_run: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate STAC Item from a single file
    ///
    /// The input can be a local file path or a remote URL (http://, https://)
    Item {
        /// Input file path or URL
        input: String,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// STAC Item ID
        #[arg(long)]
        id: Option<String>,

        /// Item title
        #[arg(long)]
        title: Option<String>,

        /// Item description
        #[arg(short, long)]
        description: Option<String>,

        /// Parent collection ID
        #[arg(short, long)]
        collection: Option<String>,

        /// Base URL for asset href (e.g., "https://example.com/data/")
        /// If provided, asset hrefs will be absolute URLs
        #[arg(long)]
        base_url: Option<String>,

        /// Pretty-print JSON
        #[arg(long, default_value_t = true)]
        pretty: bool,
    },

    /// Generate STAC Collection from directory
    Collection {
        /// Input paths (directories, files, or glob patterns like "data/*.json")
        #[arg(num_args = 0..)]
        inputs: Vec<PathBuf>,

        /// Output directory
        #[arg(short, long, default_value = "./stac_output")]
        output: PathBuf,

        /// YAML configuration file for collection metadata
        #[arg(short = 'C', long)]
        config: Option<PathBuf>,

        /// Collection ID
        #[arg(long)]
        id: Option<String>,

        /// Collection title
        #[arg(long)]
        title: Option<String>,

        /// Collection description
        #[arg(short, long)]
        description: Option<String>,

        /// Data license
        #[arg(short, long, default_value = "proprietary")]
        license: String,

        /// Glob patterns to include (e.g., "*.json", "*.jsonl")
        #[arg(long)]
        include: Vec<String>,

        /// Glob patterns to exclude (e.g., "*test*", "*.bak")
        #[arg(long)]
        exclude: Vec<String>,

        /// Scan subdirectories recursively
        #[arg(short, long, default_value_t = true)]
        recursive: bool,

        /// Maximum directory depth
        #[arg(long)]
        max_depth: Option<usize>,

        /// Skip files with errors
        #[arg(long, default_value_t = true)]
        skip_errors: bool,

        /// Base URL for asset href (e.g., "https://example.com/data/")
        /// If provided, asset hrefs will be absolute URLs
        #[arg(long)]
        base_url: Option<String>,

        /// Pretty-print JSON
        #[arg(long, default_value_t = true)]
        pretty: bool,

        /// Overwrite existing item files
        #[arg(long)]
        overwrite_items: bool,

        /// Overwrite existing collection file
        #[arg(long)]
        overwrite_collection: bool,

        /// Overwrite all (items and collection)
        #[arg(long)]
        overwrite: bool,

        /// Generate STAC GeoParquet file (items.parquet) alongside JSON output
        #[arg(long)]
        geoparquet: bool,

        /// Maximum number of files to process concurrently
        #[arg(long)]
        concurrency: Option<usize>,

        /// Maximum number of per-item links to include in collection.json (`0` disables them)
        #[arg(long)]
        max_item_links: Option<usize>,
    },

    /// Generate STAC Collection from a list of existing STAC item files
    ///
    /// This command is useful when STAC items are generated individually (e.g., for
    /// assets stored in Object Storage) and then need to be aggregated into a collection.
    /// It reads the CityJSON extension properties from each item and merges them.
    #[command(visible_alias = "aggregate")]
    UpdateCollection {
        /// STAC item JSON files to aggregate
        #[arg(required_unless_present = "items_from_file")]
        items: Vec<PathBuf>,

        /// Read item file paths from a text file (one path per line).
        /// Use this when the number of items exceeds shell argument limits.
        #[arg(long)]
        items_from_file: Option<PathBuf>,

        /// Output file path for the collection (collection.json)
        #[arg(short, long, default_value = "./collection.json")]
        output: PathBuf,

        /// YAML configuration file for collection metadata
        #[arg(short = 'C', long)]
        config: Option<PathBuf>,

        /// Collection ID
        #[arg(long)]
        id: Option<String>,

        /// Collection title
        #[arg(long)]
        title: Option<String>,

        /// Collection description
        #[arg(short, long)]
        description: Option<String>,

        /// Data license
        #[arg(short, long, default_value = "proprietary")]
        license: String,

        /// Base URL for item links (e.g., "https://example.com/stac/items/")
        /// If provided, item links will be absolute URLs
        #[arg(long)]
        items_base_url: Option<String>,

        /// Skip items with parsing errors
        #[arg(long, default_value_t = true)]
        skip_errors: bool,

        /// Pretty-print JSON
        #[arg(long, default_value_t = true)]
        pretty: bool,

        /// Generate STAC GeoParquet file (items.parquet) alongside JSON output
        #[arg(long)]
        geoparquet: bool,

        /// Maximum number of per-item links to include in collection.json (`0` disables them)
        #[arg(long)]
        max_item_links: Option<usize>,
    },

    /// Generate STAC Catalog from existing collection.json files
    ///
    /// This command builds a catalog.json from pre-existing collection.json files
    /// without re-generating items or collections. Useful when collections have
    /// already been generated (e.g., via `update-collection`) and you just need
    /// to assemble the root catalog.
    #[command(visible_alias = "aggregate-catalog")]
    UpdateCatalog {
        /// Paths to collection.json files or directories containing them
        #[arg(num_args = 0..)]
        inputs: Vec<PathBuf>,

        /// Output directory for the catalog
        #[arg(short, long, default_value = "./catalog")]
        output: PathBuf,

        /// YAML/TOML configuration file for catalog metadata
        #[arg(short = 'C', long)]
        config: Option<PathBuf>,

        /// Catalog ID (defaults to output directory name)
        #[arg(long)]
        id: Option<String>,

        /// Catalog title
        #[arg(long)]
        title: Option<String>,

        /// Catalog description
        #[arg(short, long)]
        description: Option<String>,

        /// Base URL for catalog child links
        #[arg(long)]
        base_url: Option<String>,

        /// Pretty-print JSON
        #[arg(long, default_value_t = true)]
        pretty: bool,
    },

    /// Generate STAC Catalog from multiple directories/collections
    Catalog {
        /// Input directories (each directory will be a collection)
        #[arg(num_args = 0..)]
        inputs: Vec<PathBuf>,

        /// Output directory for the catalog
        #[arg(short, long, default_value = "./catalog")]
        output: PathBuf,

        /// YAML/TOML configuration file for catalog metadata
        #[arg(short = 'C', long)]
        config: Option<PathBuf>,

        /// Catalog ID (defaults to output directory name)
        #[arg(long)]
        id: Option<String>,

        /// Catalog title
        #[arg(long)]
        title: Option<String>,

        /// Catalog description
        #[arg(short, long)]
        description: Option<String>,

        /// Configuration for collections (license, etc.)
        /// This will be applied to all generated sub-collections
        #[arg(short, long, default_value = "proprietary")]
        license: String,

        /// Base URL for catalog child links
        #[arg(long)]
        base_url: Option<String>,

        /// Pretty-print JSON
        #[arg(long, default_value_t = true)]
        pretty: bool,

        /// Overwrite existing item files
        #[arg(long)]
        overwrite_items: bool,

        /// Overwrite existing collection files
        #[arg(long)]
        overwrite_collections: bool,

        /// Overwrite all (items, collections, and catalog)
        #[arg(long)]
        overwrite: bool,

        /// Generate STAC GeoParquet file (items.parquet) alongside JSON output
        #[arg(long)]
        geoparquet: bool,

        /// Maximum number of collections to process concurrently
        #[arg(long)]
        concurrency: Option<usize>,

        /// Maximum number of per-item links to include in each generated collection.json (`0` disables them)
        #[arg(long)]
        max_item_links: Option<usize>,
    },
}

/// Helper to create a GeoParquet asset
fn make_geoparquet_asset() -> crate::stac::Asset {
    let mut asset = crate::stac::Asset::new("./items.parquet");
    asset.title = Some("STAC GeoParquet items".to_string());
    asset.r#type = Some("application/vnd.apache.parquet".to_string());
    asset.roles = vec!["collection-mirror".to_string()];
    asset
}

/// Run the CLI application
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Set up logging based on verbosity
    if cli.verbose {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Debug)
            .init();
    } else {
        env_logger::Builder::from_default_env()
            .filter_level(log::LevelFilter::Warn)
            .init();
    }

    print_banner();

    match cli.command {
        Commands::Item {
            input,
            output,
            id,
            title,
            description,
            collection,
            base_url,
            pretty,
        } => {
            handle_item_command(
                input,
                output,
                id,
                title,
                description,
                collection,
                base_url,
                pretty,
                cli.dry_run,
            )
            .await
        }

        Commands::Collection {
            inputs,
            output,
            config,
            id,
            title,
            description,
            license,
            include,
            exclude,
            recursive,
            max_depth,
            skip_errors,
            base_url,
            pretty,
            overwrite_items,
            overwrite_collection,
            overwrite,
            geoparquet,
            concurrency,
            max_item_links,
        } => {
            // Check if no inputs provided via CLI and no config file
            if inputs.is_empty() && config.is_none() {
                // No inputs in CLI and no config file - show error
                eprintln!("Error: No inputs provided. Specify inputs via CLI arguments or in a config file.");
                eprintln!("Usage: citystac collection [OPTIONS] <INPUTS>...");
                eprintln!("       citystac collection --config <CONFIG_FILE>");
                std::process::exit(1);
            }

            handle_collection_command(CollectionConfig {
                inputs,
                output,
                config,
                id,
                title,
                description,
                license,
                include,
                exclude,
                recursive,
                max_depth,
                skip_errors,
                base_url,
                pretty,
                dry_run: cli.dry_run,
                overwrite_items: overwrite_items || overwrite,
                overwrite_collection: overwrite_collection || overwrite,
                geoparquet,
                concurrency,
                max_item_links,
                parent_href: None,
                root_href: None,
            })
            .await
        }

        Commands::UpdateCollection {
            items,
            items_from_file,
            output,
            config,
            id,
            title,
            description,
            license,
            items_base_url,
            skip_errors,
            pretty,
            geoparquet,
            max_item_links,
        } => {
            let mut all_items = items;
            if let Some(list_path) = items_from_file {
                let content =
                    std::fs::read_to_string(&list_path).map_err(CityJsonStacError::IoError)?;
                for line in content.lines() {
                    let line = line.trim();
                    if !line.is_empty() {
                        all_items.push(PathBuf::from(line));
                    }
                }
            }
            handle_update_collection_command(UpdateCollectionConfig {
                items: all_items,
                output,
                config,
                id,
                title,
                description,
                license,
                items_base_url,
                skip_errors,
                pretty,
                dry_run: cli.dry_run,
                geoparquet,
                max_item_links,
            })
        }

        Commands::UpdateCatalog {
            inputs,
            output,
            config,
            id,
            title,
            description,
            base_url,
            pretty,
        } => handle_update_catalog_command(UpdateCatalogConfig {
            inputs,
            output,
            config,
            id,
            title,
            description,
            base_url,
            pretty,
        }),

        Commands::Catalog {
            inputs,
            output,
            config,
            id,
            title,
            description,
            license,
            base_url,
            pretty,
            overwrite_items,
            overwrite_collections,
            overwrite,
            geoparquet,
            concurrency,
            max_item_links,
        } => {
            handle_catalog_command(CatalogConfig {
                inputs,
                output,
                config,
                id,
                title,
                description,
                license,
                base_url,
                pretty,
                dry_run: cli.dry_run,
                overwrite_items: overwrite_items || overwrite,
                overwrite_collections: overwrite_collections || overwrite,
                geoparquet,
                concurrency,
                max_item_links,
            })
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_item_command(
    input: String,
    output: Option<PathBuf>,
    id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    collection: Option<String>,
    base_url: Option<String>,
    pretty: bool,
    dry_run: bool,
) -> Result<()> {
    // Dry-run mode: validate only
    if dry_run {
        use crate::validation;
        use progress::{print_banner, print_error, print_success};

        print_banner();

        println!("\nRunning in dry-run mode...\n");

        let result = validation::validate_item_input(&input).await?;

        println!();

        if result.is_valid() {
            print_success("Dry run complete: All validations passed");
            std::process::exit(0);
        } else {
            print_error("Dry run failed: Errors found");
            std::process::exit(result.exit_code());
        }
    }

    // Parse input as either local file or remote URL
    let spinner = create_spinner(format!("Reading {input}…"));
    let source = InputSource::from_str_input(&input)?;
    let reader = match get_reader_from_source(&source).await {
        Ok(r) => r,
        Err(e) => {
            finish_spinner_err(spinner, format!("Failed to read input: {e}"));
            return Err(e);
        }
    };
    finish_spinner_ok(
        spinner,
        format!("Loaded {} ({} format)", input, reader.encoding()),
    );

    let spinner = create_spinner("Building STAC Item…");

    // Build STAC Item
    // For remote URLs, use the original URL as the asset href when no base_url is given
    let original_url = match &source {
        InputSource::Remote(url) => Some(url.as_str()),
        InputSource::Local(_) => None,
    };
    let mut builder = StacItemBuilder::from_file(
        reader.file_path(),
        reader.as_ref(),
        base_url.as_deref(),
        original_url,
    )?;

    // Apply custom options
    if let Some(custom_id) = id {
        let props = crate::adapter::properties_from_reader(reader.as_ref())?;
        let resolved_crs = StacItemBuilder::resolve_crs(reader.as_ref(), None);
        builder = StacItemBuilder::new(custom_id)
            .datetime_from_reference_date(reader.metadata().ok().flatten().as_ref())
            .city3d(props)?
            .crs(&resolved_crs);

        if let Ok(bbox) = reader.bbox() {
            let crs = reader.crs().unwrap_or_default();
            let wgs84_bbox = bbox.to_wgs84(&crs)?;
            builder = builder.bbox(wgs84_bbox).geometry_from_bbox();
        }
    }

    if let Some(t) = title {
        builder = builder.title(t);
    }

    if let Some(d) = description {
        builder = builder.description(d);
    }

    // Add collection link and ID if specified
    if let Some(coll_id) = collection {
        builder = builder
            .collection_id(&coll_id)
            .collection_link(format!("./{coll_id}.json"));
    }

    // Generate output path
    let output_path = output.unwrap_or_else(|| {
        // For URLs, use a filename derived from the URL
        // For local files, use the file path with .item.json extension
        match source {
            InputSource::Local(path) => {
                let mut p = path.clone();
                p.set_extension("item.json");
                p
            }
            InputSource::Remote(url) => {
                let filename = url
                    .split('/')
                    .next_back()
                    .and_then(|s| s.split('?').next())
                    .unwrap_or("remote.item.json");
                PathBuf::from(format!("{}.json", filename.trim_end_matches(".json")))
            }
        }
    });

    // Build and serialize
    let item = builder.build()?;
    let json = if pretty {
        serde_json::to_string_pretty(&item)?
    } else {
        serde_json::to_string(&item)?
    };

    // Write output
    std::fs::write(&output_path, json)?;

    finish_spinner_ok(
        spinner,
        format!("Item written to {}", output_path.display()),
    );

    Ok(())
}

/// Configuration for catalog generation
struct CatalogConfig {
    inputs: Vec<PathBuf>,
    output: PathBuf,
    config: Option<PathBuf>,
    id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    license: String,
    base_url: Option<String>,
    pretty: bool,
    dry_run: bool,
    overwrite_items: bool,
    overwrite_collections: bool,
    geoparquet: bool,
    concurrency: Option<usize>,
    max_item_links: Option<usize>,
}

struct UpdateCatalogConfig {
    inputs: Vec<PathBuf>,
    output: PathBuf,
    config: Option<PathBuf>,
    id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    base_url: Option<String>,
    pretty: bool,
}

fn handle_update_catalog_command(config: UpdateCatalogConfig) -> Result<()> {
    use crate::config::{CatalogCliArgs, CatalogConfigFile, CollectionConfigFile};
    use crate::stac::StacCatalogBuilder;

    // Load config file if provided
    let base_config = if let Some(config_path) = &config.config {
        CatalogConfigFile::from_file(config_path)?
    } else {
        CatalogConfigFile::default()
    };

    // Merge with CLI args
    let merged_config = base_config.merge_with_cli(&CatalogCliArgs {
        id: config.id.clone(),
        title: config.title.clone(),
        description: config.description.clone(),
        base_url: config.base_url.clone(),
        concurrency: None, // update-catalog does not process items
    });

    // Collect collection.json paths from inputs + config
    let mut collection_paths: Vec<PathBuf> = Vec::new();

    for input in &config.inputs {
        if input.is_file() {
            collection_paths.push(input.clone());
        } else if input.is_dir() {
            let candidate = input.join("collection.json");
            if candidate.exists() {
                collection_paths.push(candidate);
            } else {
                print_warning(format!("No collection.json found in {}", input.display()));
            }
        } else {
            print_warning(format!("Path not found: {}", input.display()));
        }
    }

    // Process config collections
    if let Some(config_collections) = &merged_config.collections {
        let base_dir = config
            .config
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new("."));

        for coll_path_str in config_collections {
            let path = base_dir.join(coll_path_str);
            if path.is_file() {
                // catalog-config.yaml may reference either an existing
                // collection.json or a collection-config YAML (the same schema
                // the `catalog` command consumes). For the YAML case, read the
                // config to learn the collection id, then look up the
                // previously generated collection.json under <output>/<id>/.
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_ascii_lowercase())
                    .unwrap_or_default();
                if ext == "yaml" || ext == "yml" {
                    match CollectionConfigFile::from_file(&path) {
                        Ok(cfg) => {
                            if let Some(id) = cfg.id {
                                let candidate = config.output.join(&id).join("collection.json");
                                if candidate.exists() {
                                    collection_paths.push(candidate);
                                } else {
                                    print_warning(format!(
                                        "Collection config {} declares id '{}' but no collection.json found at {}",
                                        path.display(),
                                        id,
                                        candidate.display()
                                    ));
                                }
                            } else {
                                print_warning(format!(
                                    "Collection config {} has no id; cannot locate its generated collection.json",
                                    path.display()
                                ));
                            }
                        }
                        Err(e) => {
                            print_warning(format!(
                                "Failed to parse collection config {}: {}",
                                path.display(),
                                e
                            ));
                        }
                    }
                } else {
                    collection_paths.push(path);
                }
            } else if path.is_dir() {
                let candidate = path.join("collection.json");
                if candidate.exists() {
                    collection_paths.push(candidate);
                } else {
                    print_warning(format!("No collection.json found in {}", path.display()));
                }
            } else {
                print_warning(format!("Path not found: {}", path.display()));
            }
        }
    }

    if collection_paths.is_empty() {
        print_error("No collection.json files found. Provide paths to collection.json files or directories containing them.");
        std::process::exit(1);
    }

    print_info(format!(
        "Reading {} existing collection(s)",
        collection_paths.len()
    ));

    // Read each collection.json and extract id + title
    let mut generated_collections: Vec<(String, String)> = Vec::new(); // (href, title)
    let mut errors: u64 = 0;

    // Create output directory
    std::fs::create_dir_all(&config.output)?;

    for coll_path in &collection_paths {
        let content = match std::fs::read_to_string(coll_path) {
            Ok(c) => c,
            Err(e) => {
                print_warning(format!("Failed to read {}: {}", coll_path.display(), e));
                errors += 1;
                continue;
            }
        };

        let mut collection: serde_json::Value = match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                print_warning(format!("Failed to parse {}: {}", coll_path.display(), e));
                errors += 1;
                continue;
            }
        };

        let col_id = collection
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let col_title = collection
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or(&col_id)
            .to_string();

        // Update parent and root links in the collection
        if let Some(links) = collection.get_mut("links").and_then(|v| v.as_array_mut()) {
            // Remove existing parent and root links
            links.retain(|link| {
                let rel = link.get("rel").and_then(|v| v.as_str()).unwrap_or("");
                rel != "parent" && rel != "root"
            });

            // Compute relative path from collection.json to catalog.json
            let catalog_rel_href = if let Some(base) = &config.base_url {
                let normalized = if base.ends_with('/') {
                    base.to_string()
                } else {
                    format!("{base}/")
                };
                format!("{normalized}catalog.json")
            } else {
                // Collections are typically one directory below the catalog
                "../catalog.json".to_string()
            };

            links.push(serde_json::json!({
                "rel": "parent",
                "href": catalog_rel_href,
                "type": "application/json"
            }));
            links.push(serde_json::json!({
                "rel": "root",
                "href": catalog_rel_href,
                "type": "application/json"
            }));
        }

        // Write back the updated collection.json
        let updated_json = if config.pretty {
            serde_json::to_string_pretty(&collection)?
        } else {
            serde_json::to_string(&collection)?
        };
        std::fs::write(coll_path, updated_json)?;

        // Compute href relative to catalog output directory
        let href = if let Some(base) = &config.base_url {
            let normalized_base = if base.ends_with('/') {
                base.to_string()
            } else {
                format!("{base}/")
            };
            format!("{normalized_base}{col_id}/collection.json")
        } else if let Ok(rel) = coll_path.strip_prefix(&config.output) {
            format!("./{}", rel.display())
        } else if let Ok(abs_coll) = coll_path.canonicalize() {
            if let Ok(abs_out) = config.output.canonicalize() {
                if let Ok(rel) = abs_coll.strip_prefix(&abs_out) {
                    format!("./{}", rel.display())
                } else {
                    format!("./{col_id}/collection.json")
                }
            } else {
                format!("./{col_id}/collection.json")
            }
        } else {
            format!("./{col_id}/collection.json")
        };

        println!("  {} {}", console::style("✓").green(), col_title);
        generated_collections.push((href, col_title));
    }

    // Build catalog
    let catalog_id = merged_config.id.unwrap_or_else(|| {
        config
            .output
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("catalog")
            .to_string()
    });

    let description = merged_config
        .description
        .unwrap_or_else(|| "Root catalog".to_string());

    let mut catalog_builder = StacCatalogBuilder::new(catalog_id, description);

    if let Some(t) = merged_config.title {
        catalog_builder = catalog_builder.title(t);
    }

    let collection_count = generated_collections.len();
    for (href, title) in generated_collections {
        catalog_builder = catalog_builder.child_link(href, Some(title));
    }

    catalog_builder = catalog_builder
        .self_link("./catalog.json")
        .root_link("./catalog.json");

    let catalog = catalog_builder.build();
    let catalog_json = if config.pretty {
        serde_json::to_string_pretty(&catalog)?
    } else {
        serde_json::to_string(&catalog)?
    };

    let catalog_path = config.output.join("catalog.json");
    std::fs::write(&catalog_path, &catalog_json)?;

    Summary::new()
        .add("Catalog", catalog_path.display().to_string())
        .add("Collections", format!("{collection_count}"))
        .add("Errors", format!("{errors}"))
        .print();
    print_success("Catalog generated successfully");

    Ok(())
}

/// Sanitize a string for use as a folder name by replacing invalid characters with underscores
fn sanitize_folder_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Extract a folder name from a path string (filename stem)
fn fallback_folder_name(path_str: &str) -> String {
    std::path::Path::new(path_str)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("collection")
        .to_string()
}

/// Rewrite a collection.json's `parent` and `root` links to point at the given
/// catalog href. Existing parent/root links are dropped first so re-running is
/// idempotent. Used by the `catalog` handler so that catalog membership lands
/// on every staged collection.json — including ones that errored out during
/// per-collection regeneration (e.g., transient remote-fetch failures).
///
/// Also reconciles the `items-geoparquet` collection asset against the
/// presence of a sibling `items.parquet`: a partial-rewrite path (catalog
/// refresh) can otherwise silently drop the asset reference even though the
/// parquet file is right there on disk.
fn refresh_parent_root_links(coll_path: &Path, catalog_href: &str, pretty: bool) -> Result<()> {
    let content = std::fs::read_to_string(coll_path)?;
    let mut collection: crate::stac::StacCollection = serde_json::from_str(&content)?;
    collection
        .links
        .retain(|l| l.rel != "parent" && l.rel != "root");
    collection.links.push(stac::Link::parent(catalog_href));
    collection.links.push(stac::Link::root(catalog_href));

    let parquet_path = coll_path
        .parent()
        .map(|p| p.join("items.parquet"))
        .unwrap_or_else(|| PathBuf::from("items.parquet"));
    if parquet_path.exists() {
        collection
            .assets
            .entry("items-geoparquet".to_string())
            .or_insert_with(make_geoparquet_asset);
    }

    let updated_json = if pretty {
        serde_json::to_string_pretty(&collection)?
    } else {
        serde_json::to_string(&collection)?
    };
    std::fs::write(coll_path, updated_json)?;
    Ok(())
}

async fn handle_catalog_command(config: CatalogConfig) -> Result<()> {
    use crate::config::{CatalogCliArgs, CatalogConfigFile};
    use crate::stac::StacCatalogBuilder;

    // Dry-run mode: validate only
    if config.dry_run {
        use progress::{print_banner, print_error, print_success};

        print_banner();

        println!("\nRunning in dry-run mode...\n");

        // Validate config file if provided
        if let Some(config_path) = &config.config {
            println!("  → Checking config file: {}", config_path.display());
            match CatalogConfigFile::from_file(config_path) {
                Ok(catalog_config) => {
                    println!("  ✓ Config file syntax: valid");

                    // Validate semantic content
                    let mut semantic_errors = Vec::new();

                    if catalog_config.id.is_none()
                        || catalog_config
                            .id
                            .as_ref()
                            .map(|s| s.trim())
                            .unwrap_or_default()
                            .is_empty()
                    {
                        semantic_errors.push("Missing required field: 'id'".to_string());
                    }

                    if catalog_config.title.is_none()
                        || catalog_config
                            .title
                            .as_ref()
                            .map(|s| s.trim())
                            .unwrap_or_default()
                            .is_empty()
                    {
                        semantic_errors.push("Missing recommended field: 'title'".to_string());
                    }

                    if catalog_config.description.is_none()
                        || catalog_config
                            .description
                            .as_ref()
                            .map(|s| s.trim())
                            .unwrap_or_default()
                            .is_empty()
                    {
                        semantic_errors
                            .push("Missing recommended field: 'description'".to_string());
                    }

                    if !semantic_errors.is_empty() {
                        for error in &semantic_errors {
                            println!("  ✗ {}", error);
                        }
                        println!();
                        print_error("Dry run failed: Config semantic errors");
                        std::process::exit(1);
                    }

                    println!("  ✓ Config file content: valid");
                }
                Err(e) => {
                    println!("  ✗ Config file syntax: {}", e);
                    println!();
                    print_error("Dry run failed: Config error");
                    std::process::exit(1);
                }
            }
        }

        // Validate input directories/collections
        let mut found = 0;
        let mut missing = Vec::new();

        for input in &config.inputs {
            if input.exists() {
                found += 1;
            } else {
                missing.push(input.clone());
            }
        }

        if missing.is_empty() {
            println!("  ✓ Input paths: {}/{} found", found, config.inputs.len());
        } else {
            println!("  ⚠ Input paths: {}/{} found", found, config.inputs.len());
            for path in &missing {
                println!("    ✗ {}", path.display());
            }
        }

        println!();

        if missing.is_empty() {
            print_success("Dry run complete: All validations passed");
            std::process::exit(0);
        } else {
            print_error("Dry run failed: Missing paths");
            std::process::exit(2);
        }
    }

    // Load config file if provided
    let base_config = if let Some(config_path) = &config.config {
        CatalogConfigFile::from_file(config_path)?
    } else {
        CatalogConfigFile::default()
    };

    // Merge with CLI args
    let merged_config = base_config.merge_with_cli(&CatalogCliArgs {
        id: config.id.clone(),
        title: config.title.clone(),
        description: config.description.clone(),
        base_url: config.base_url.clone(),
        concurrency: config.concurrency,
    });

    // Create output directory
    std::fs::create_dir_all(&config.output)?;

    // Determine collections to process
    let mut collection_targets: Vec<(PathBuf, String)> = Vec::new(); // (path, id_hint)

    // Process CLI inputs (directories)
    for input in &config.inputs {
        let id_hint = input
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("collection")
            .to_string();
        collection_targets.push((input.clone(), id_hint));
    }

    // Process config collections
    if let Some(config_collections) = merged_config.collections {
        // Resolve paths relative to config file if provided, otherwise CWD
        let base_dir = config
            .config
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new("."));

        for coll_path_str in config_collections {
            let path = base_dir.join(&coll_path_str);

            // Try to read the id from the config file for the folder name
            let id_hint = if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if matches!(ext, "toml" | "yaml" | "yml") {
                        // Try to parse the config file to get its id
                        match CollectionConfigFile::from_file(&path) {
                            Ok(cfg) => {
                                if let Some(id) = cfg.id {
                                    // Sanitize the id for use as a folder name
                                    sanitize_folder_name(&id)
                                } else {
                                    // No id in config, fall back to filename
                                    fallback_folder_name(&coll_path_str)
                                }
                            }
                            Err(_) => {
                                // Failed to parse, fall back to filename
                                fallback_folder_name(&coll_path_str)
                            }
                        }
                    } else {
                        fallback_folder_name(&coll_path_str)
                    }
                } else {
                    fallback_folder_name(&coll_path_str)
                }
            } else {
                // Directory: use directory name
                fallback_folder_name(&coll_path_str)
            };
            collection_targets.push((path, id_hint));
        }
    }

    if collection_targets.is_empty() {
        print_error("No collections provided. Specify input directories via CLI or 'collections' in config file.");
        std::process::exit(1);
    }

    print_info(format!(
        "Processing {} collection(s) for catalog",
        collection_targets.len()
    ));

    let total_collections = collection_targets.len() as u64;
    let catalog_pb = create_progress_bar(total_collections, "Generating collections…");
    let catalog_pb_arc = std::sync::Arc::new(catalog_pb);

    // Capture id_hints before the stream consumes collection_targets — used
    // after per-collection processing to refresh parent/root links on every
    // staged collection.json regardless of whether regeneration succeeded.
    let collection_id_hints: Vec<String> = collection_targets
        .iter()
        .map(|(_, id)| id.clone())
        .collect();

    let mut generated_collections: Vec<(String, String)> = Vec::new(); // (href, title)
    let mut catalog_errors: u64 = 0;

    // Process collections concurrently
    let config_output = config.output.clone();
    let config_base_url = config.base_url.clone();
    let config_license = config.license.clone();
    let config_pretty = config.pretty;
    let config_dry_run = config.dry_run;
    let config_overwrite_items = config.overwrite_items;
    let config_overwrite_collections = config.overwrite_collections;
    let config_geoparquet = config.geoparquet;

    // CLI flag took precedence in merge_with_cli; YAML value is used as fallback.
    let catalog_concurrency = merged_config
        .concurrency
        .filter(|&n| n > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });

    // Use buffer_unordered to limit both concurrency and memory usage.
    // Unlike spawn-all + join_all, this only keeps `catalog_concurrency` collections
    // in-flight at a time, preventing memory spikes from multiple large collections.
    use futures::stream::{self, StreamExt};

    let mut result_stream = stream::iter(collection_targets)
        .map(|(input_dir, id_hint)| {
            let pb = catalog_pb_arc.clone();
            let output = config_output.clone();
            let base_url = config_base_url.clone();
            let license = config_license.clone();

            async move {
                if !input_dir.exists() {
                    pb.println(format!(
                        "  {} Directory not found, skipping: {}",
                        console::style("⚠").yellow(),
                        input_dir.display()
                    ));
                    pb.inc(1);
                    return Err((
                        input_dir.display().to_string(),
                        "Directory not found".to_string(),
                    ));
                }

                let collection_output_dir = output.join(&id_hint);

                let mut collection_config = CollectionConfig {
                    inputs: Vec::new(),
                    output: collection_output_dir,
                    config: None,
                    id: Some(id_hint.clone()),
                    title: Some(format!("Collection from {}", id_hint)),
                    description: None,
                    license,
                    include: vec![],
                    exclude: vec![],
                    recursive: true,
                    max_depth: None,
                    skip_errors: true,
                    base_url: None,
                    pretty: config_pretty,
                    dry_run: config_dry_run,
                    overwrite_items: config_overwrite_items,
                    overwrite_collection: config_overwrite_collections,
                    geoparquet: config_geoparquet,
                    concurrency: config.concurrency,
                    max_item_links: config.max_item_links,
                    parent_href: Some("../catalog.json".to_string()),
                    root_href: Some("../catalog.json".to_string()),
                };

                // Check if input is a config file
                if input_dir.is_file() {
                    if let Some(ext) = input_dir.extension().and_then(|e| e.to_str()) {
                        if matches!(ext, "toml" | "yaml" | "yml") {
                            pb.println(format!(
                                "  {} Loading config: {}",
                                console::style("›").blue(),
                                input_dir.display()
                            ));
                            collection_config.config = Some(input_dir.clone());
                        } else {
                            collection_config.inputs = vec![input_dir.clone()];
                            collection_config.base_url =
                                base_url.clone().map(|u| format!("{u}{id_hint}/"));
                        }
                    } else {
                        collection_config.inputs = vec![input_dir.clone()];
                        collection_config.base_url =
                            base_url.clone().map(|u| format!("{u}{id_hint}/"));
                    }
                } else {
                    collection_config.inputs = vec![input_dir.clone()];
                    collection_config.base_url = base_url.clone().map(|u| format!("{u}{id_hint}/"));
                }

                pb.set_message(format!("Processing: {id_hint}"));
                match process_collection_logic(collection_config).await {
                    Ok((_col_path, col_id, col_title)) => {
                        let relative_href = format!("./{}/collection.json", id_hint);

                        let href = if let Some(base) = &base_url {
                            let normalized_base = if base.ends_with('/') {
                                base.to_string()
                            } else {
                                format!("{base}/")
                            };
                            format!("{normalized_base}{id_hint}/collection.json")
                        } else {
                            relative_href
                        };

                        pb.println(format!(
                            "  {} Collection ready: {}",
                            console::style("✓").green(),
                            col_title.clone().unwrap_or_else(|| col_id.clone())
                        ));
                        pb.inc(1);
                        Ok((href, col_title.unwrap_or(col_id)))
                    }
                    Err(e) => {
                        pb.println(format!(
                            "  {} Failed ({}): {}",
                            console::style("✗").red(),
                            input_dir.display(),
                            e
                        ));
                        pb.inc(1);
                        Err((input_dir.display().to_string(), e.to_string()))
                    }
                }
            }
        })
        .buffer_unordered(catalog_concurrency);

    // Process results as they complete
    while let Some(result) = result_stream.next().await {
        match result {
            Ok((href, title)) => {
                generated_collections.push((href, title));
            }
            Err(_) => {
                catalog_errors += 1;
            }
        }
    }
    catalog_pb_arc.finish_and_clear();

    // Refresh parent/root links on every staged collection.json. This runs
    // regardless of per-collection processing outcomes — so collections that
    // erred out (missing input dir, transient remote-fetch failures, etc.)
    // still pick up catalog membership as long as a prior collection.json is
    // on disk.
    for id_hint in &collection_id_hints {
        let coll_path = config.output.join(id_hint).join("collection.json");
        if !coll_path.exists() {
            continue;
        }
        if let Err(e) = refresh_parent_root_links(&coll_path, "../catalog.json", config.pretty) {
            print_warning(format!(
                "Failed to refresh parent/root links on {}: {}",
                coll_path.display(),
                e
            ));
        }
    }

    // Generate Catalog
    let catalog_id = merged_config.id.unwrap_or_else(|| {
        config
            .output
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("catalog")
            .to_string()
    });

    let description = merged_config
        .description
        .unwrap_or_else(|| "Root catalog".to_string());

    let mut catalog_builder = StacCatalogBuilder::new(catalog_id, description);

    if let Some(t) = merged_config.title {
        catalog_builder = catalog_builder.title(t);
    }

    let collection_count = generated_collections.len();
    for (href, title) in generated_collections {
        catalog_builder = catalog_builder.child_link(href, Some(title));
    }

    catalog_builder = catalog_builder
        .self_link("./catalog.json")
        .root_link("./catalog.json");

    let catalog = catalog_builder.build();
    let catalog_json = if config.pretty {
        serde_json::to_string_pretty(&catalog)?
    } else {
        serde_json::to_string(&catalog)?
    };

    let catalog_path = config.output.join("catalog.json");
    std::fs::write(&catalog_path, catalog_json)?;

    Summary::new()
        .add("Catalog", catalog_path.display().to_string())
        .add("Collections", format!("{collection_count}"))
        .add("Errors", format!("{catalog_errors}"))
        .print();
    print_success("Catalog generated successfully");

    Ok(())
}

/// Configuration for collection generation
struct CollectionConfig {
    inputs: Vec<PathBuf>,
    output: PathBuf,
    config: Option<PathBuf>,
    id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    license: String,
    include: Vec<String>,
    exclude: Vec<String>,
    recursive: bool,
    max_depth: Option<usize>,
    skip_errors: bool,
    base_url: Option<String>,
    pretty: bool,
    dry_run: bool,
    overwrite_items: bool,
    overwrite_collection: bool,
    geoparquet: bool,
    concurrency: Option<usize>,
    max_item_links: Option<usize>,
    /// Parent link href (set when collection is part of a catalog)
    parent_href: Option<String>,
    /// Root link href (set when collection is part of a catalog)
    root_href: Option<String>,
}

async fn handle_collection_command(config: CollectionConfig) -> Result<()> {
    // Dry-run mode: validate only
    if config.dry_run {
        use crate::validation;
        use progress::{print_banner, print_error, print_success};

        print_banner();

        println!("\nRunning in dry-run mode...\n");

        // Determine final inputs
        let base_config = if let Some(config_path) = &config.config {
            // Load config to validate it
            let _base_config = CollectionConfigFile::from_file(config_path)?;
            validation::validate_collection_config(
                &Some(config_path.clone()),
                &config.inputs,
                &config.base_url,
            )
            .await?
        } else {
            validation::validate_collection_config(&None, &config.inputs, &config.base_url).await?
        };

        println!();

        // Print final status
        if base_config.is_valid() {
            print_success("Dry run complete: All validations passed");
            std::process::exit(0);
        } else {
            print_error("Dry run failed: Errors found");
            std::process::exit(base_config.exit_code());
        }
    }

    match process_collection_logic(config).await {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

async fn process_collection_logic(
    config: CollectionConfig,
) -> Result<(PathBuf, String, Option<String>)> {
    use crate::stac::{CollectionAccumulator, ItemMetadata};

    // Load config file if provided
    let base_config = if let Some(config_path) = &config.config {
        CollectionConfigFile::from_file(config_path)?
    } else {
        CollectionConfigFile::default()
    };

    // Merge with CLI args
    let merged_config = base_config.merge_with_cli(&CollectionCliArgs {
        id: config.id.clone(),
        title: config.title.clone(),
        description: config.description.clone(),
        license: if config.license != "proprietary" {
            Some(config.license.clone())
        } else {
            None
        },
        base_url: config.base_url.clone(),
        concurrency: config.concurrency,
    });

    // Determine final inputs: CLI inputs take precedence, fall back to config inputs
    let final_inputs = if !config.inputs.is_empty() {
        // CLI inputs provided - use them
        config.inputs.clone()
    } else if let Some(config_inputs) = merged_config.inputs {
        // No CLI inputs, but config file has inputs
        // Resolve the inputs (may need to read from file if using from_file)
        let config_dir = config
            .config
            .as_ref()
            .and_then(|p| p.parent())
            .unwrap_or(Path::new("."));
        let resolved_inputs = config_inputs.resolve(config_dir)?;
        resolved_inputs
            .iter()
            .map(|s| PathBuf::from(s.as_str()))
            .collect()
    } else {
        // No inputs — may be a config-only collection (e.g., Helsinki viewer-only)
        Vec::new()
    };

    // Extract CRS override from config (used as fallback when files lack CRS metadata)
    let crs_override: Option<CRS> = merged_config
        .extent
        .as_ref()
        .and_then(|e| e.spatial.as_ref())
        .and_then(|s| s.crs.as_ref())
        .and_then(|crs_str| CRS::from_citygml_srs_name(crs_str));

    // Determine collection ID early so items can reference it
    let collection_id = merged_config.id.clone().unwrap_or_else(|| {
        final_inputs
            .first()
            .and_then(|p| p.file_name().and_then(|n| n.to_str()))
            .unwrap_or("collection")
            .to_string()
    });

    // Check for remote URLs vs local files
    let mut sources: Vec<InputSource> = Vec::new();
    let mut local_search_paths: Vec<PathBuf> = Vec::new();

    for input in &final_inputs {
        let input_str = input.to_string_lossy();
        if crate::remote::is_remote_url(&input_str) {
            sources.push(InputSource::Remote(input_str.to_string()));
        } else {
            local_search_paths.push(input.clone());
        }
    }

    log::info!(
        "Scanning {} local path(s) and {} remote URL(s)",
        local_search_paths.len(),
        sources.len()
    );

    // Find all supported files in local search paths
    if !local_search_paths.is_empty() {
        let files = traversal::find_files_with_patterns(
            &local_search_paths,
            &config.include,
            &config.exclude,
            config.recursive,
            config.max_depth,
        )?;

        // Add found local files to sources
        for file in files {
            sources.push(InputSource::Local(file));
        }
    }

    // Check if this is a config-only collection (no input files, metadata from config)
    let config_only = sources.is_empty();

    if config_only {
        // Config-only mode: need at least a bbox from config
        let has_config_bbox = merged_config
            .extent
            .as_ref()
            .and_then(|e| e.spatial.as_ref())
            .and_then(|s| s.bbox.as_ref())
            .is_some_and(|bbox| !bbox.is_empty());

        if !has_config_bbox {
            return Err(crate::error::CityJsonStacError::StacError(
                "No input files found and no spatial extent (bbox) in config. \
                 For collection-only mode, provide extent.spatial.bbox in the config file."
                    .to_string(),
            ));
        }

        print_info("Config-only mode: generating collection from config metadata (no items)");
    } else {
        print_info(format!("Found {} input source(s)", sources.len()));
    }
    log_memory(format!(
        "collection-start id={} sources={}",
        collection_id,
        sources.len()
    ));

    // --- File processing (skipped in config-only mode) ---
    let mut stem_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();

    // Pre-scan sources to count filenames for collision detection
    for source in &sources {
        let filename = match source {
            InputSource::Local(p) => p
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string(),
            InputSource::Remote(u) => crate::remote::url_filename(u),
        };
        // Get stem (remove extension)
        let path = PathBuf::from(&filename);
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");
        *stem_counts.entry(stem.to_string()).or_insert(0) += 1;
    }

    // Create output directories
    std::fs::create_dir_all(&config.output)?;
    let items_dir = config.output.join("items");
    if !config_only {
        std::fs::create_dir_all(&items_dir)?;
    }

    // Accumulator for streaming processing
    let mut accumulator = CollectionAccumulator::new(config.max_item_links);
    let memory_log_every = memory_log_interval(1000);

    // Process each file concurrently - write items immediately, accumulate metadata
    let pb = create_progress_bar(sources.len() as u64, "Processing files…");

    // Shared state for concurrent processing
    let pb_arc = std::sync::Arc::new(pb);
    let items_dir_arc = std::sync::Arc::new(items_dir.clone());
    let base_url_arc = std::sync::Arc::new(config.base_url.clone());
    let collection_id_arc = std::sync::Arc::new(collection_id.clone());
    let crs_override_arc = std::sync::Arc::new(crs_override.clone());
    let stem_counts_arc = std::sync::Arc::new(stem_counts);

    /// Result of processing a single item concurrently
    enum ItemResult {
        /// Successfully processed item
        Success {
            metadata: ItemMetadata,
            item_href: String,
            title: Option<String>,
        },
        /// Item processing failed
        Error { source: String, error: String },
        /// Non-recoverable error (when skip_errors is false)
        Fatal(CityJsonStacError),
    }

    // CLI flag took precedence in merge_with_cli; YAML value is used as fallback.
    let concurrency_limit = merged_config
        .concurrency
        .filter(|&n| n > 0)
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
        });

    // Use buffer_unordered to limit both concurrency and memory usage.
    // Unlike spawn-all + join_all, this only keeps `concurrency_limit` tasks
    // in-flight at a time, avoiding OOM for large collections (e.g. 166K+ items).
    use futures::stream::{self, StreamExt};

    let skip_errors = config.skip_errors;
    let pretty = config.pretty;
    let overwrite_items = config.overwrite_items;

    let mut result_stream = stream::iter(sources)
        .map(|source| {
            let pb = pb_arc.clone();
            let items_dir = items_dir_arc.clone();
            let base_url = base_url_arc.clone();
            let collection_id = collection_id_arc.clone();
            let crs_override = crs_override_arc.clone();
            let stem_counts = stem_counts_arc.clone();

            async move {
                let source_desc = match &source {
                    InputSource::Local(p) => p.display().to_string(),
                    InputSource::Remote(u) => u.clone(),
                };
                let short_desc = source_desc
                    .split(['/', '\\'])
                    .next_back()
                    .unwrap_or(&source_desc)
                    .to_string();
                pb.set_message(format!("Processing: {short_desc}"));

                // Get the reader
                let reader = match get_reader_from_source(&source).await {
                    Ok(r) => r,
                    Err(e) => {
                        if skip_errors {
                            pb.println(format!(
                                "  {} Skipping {short_desc}: {e}",
                                console::style("⚠").yellow()
                            ));
                            pb.inc(1);
                            return ItemResult::Error {
                                source: source_desc,
                                error: e.to_string(),
                            };
                        } else {
                            pb.inc(1);
                            return ItemResult::Fatal(e);
                        }
                    }
                };

                // Determine item ID and filename
                let file_path = reader.file_path();
                let stem = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");
                let has_collision = stem_counts.get(stem).is_some_and(|&count| count > 1);

                let item_id = if has_collision {
                    let encoding = reader.encoding();
                    let suffix = match encoding {
                        "CityJSON" => "_cj",
                        "CityJSONSeq" => "_cjseq",
                        "FlatCityBuf" => "_fcb",
                        _ => "",
                    };
                    format!("{}{}", stem, suffix)
                } else {
                    stem.to_string()
                };

                let item_filename = format!("{item_id}_item.json");
                let item_path = items_dir.join(&item_filename);

                // Check if item already exists and overwrite flag
                if item_path.exists() && !overwrite_items {
                    pb.println(format!(
                        "  {} Skipping existing: {}",
                        console::style("⚠").yellow(),
                        item_filename
                    ));

                    match ItemMetadata::from_file(&item_path) {
                        Ok(metadata) => {
                            let item_href = format!("./items/{item_filename}");
                            let title = file_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(String::from);
                            pb.inc(1);
                            return ItemResult::Success {
                                metadata,
                                item_href,
                                title,
                            };
                        }
                        Err(e) => {
                            if skip_errors {
                                pb.println(format!(
                                    "  {} Failed to read existing item: {e}",
                                    console::style("✗").red()
                                ));
                                pb.inc(1);
                                return ItemResult::Error {
                                    source: item_filename,
                                    error: e,
                                };
                            } else {
                                pb.inc(1);
                                return ItemResult::Fatal(CityJsonStacError::StacError(format!(
                                    "Failed to read existing item {}: {}",
                                    item_path.display(),
                                    e
                                )));
                            }
                        }
                    }
                }

                // For remote sources, preserve the original URL as the asset href fallback
                let original_url = match &source {
                    InputSource::Remote(url) => Some(url.clone()),
                    InputSource::Local(_) => None,
                };

                // Process and generate item
                let builder_result = if has_collision {
                    StacItemBuilder::from_file_with_format_suffix_and_crs(
                        file_path,
                        reader.as_ref(),
                        base_url.as_deref(),
                        original_url.as_deref(),
                        (*crs_override).as_ref(),
                    )
                } else {
                    StacItemBuilder::from_file_with_crs_override(
                        file_path,
                        reader.as_ref(),
                        base_url.as_deref(),
                        original_url.as_deref(),
                        (*crs_override).as_ref(),
                    )
                };

                match builder_result {
                    Ok(builder) => match builder
                        .collection_id(&*collection_id)
                        .collection_link("../collection.json")
                        .build()
                    {
                        Ok(item) => {
                            let metadata = ItemMetadata::from_item(&item);
                            let item_id = item.id.clone();

                            // Serialize item
                            let json = if pretty {
                                serde_json::to_string_pretty(&item)
                            } else {
                                serde_json::to_string(&item)
                            };

                            match json {
                                Ok(json) => {
                                    let item_filename = format!("{item_id}_item.json");
                                    let item_path = items_dir.join(&item_filename);
                                    if let Err(e) = tokio::fs::write(&item_path, &json).await {
                                        if skip_errors {
                                            pb.println(format!(
                                                "  {} Skipping {short_desc}: {e}",
                                                console::style("⚠").yellow()
                                            ));
                                            pb.inc(1);
                                            return ItemResult::Error {
                                                source: source_desc,
                                                error: e.to_string(),
                                            };
                                        } else {
                                            pb.inc(1);
                                            return ItemResult::Fatal(CityJsonStacError::IoError(
                                                e,
                                            ));
                                        }
                                    }

                                    let item_href = format!("./items/{item_filename}");
                                    let title = file_path
                                        .file_name()
                                        .and_then(|n| n.to_str())
                                        .map(String::from);
                                    pb.inc(1);
                                    ItemResult::Success {
                                        metadata,
                                        item_href,
                                        title,
                                    }
                                }
                                Err(e) => {
                                    if skip_errors {
                                        pb.println(format!(
                                            "  {} Skipping {short_desc}: {e}",
                                            console::style("⚠").yellow()
                                        ));
                                        pb.inc(1);
                                        ItemResult::Error {
                                            source: source_desc,
                                            error: e.to_string(),
                                        }
                                    } else {
                                        pb.inc(1);
                                        ItemResult::Fatal(CityJsonStacError::JsonError(e))
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            if skip_errors {
                                pb.println(format!(
                                    "  {} Skipping {short_desc}: {e}",
                                    console::style("⚠").yellow()
                                ));
                                pb.inc(1);
                                ItemResult::Error {
                                    source: source_desc,
                                    error: e.to_string(),
                                }
                            } else {
                                pb.inc(1);
                                ItemResult::Fatal(e)
                            }
                        }
                    },
                    Err(e) => {
                        if skip_errors {
                            pb.println(format!(
                                "  {} Skipping {short_desc}: {e}",
                                console::style("⚠").yellow()
                            ));
                            pb.inc(1);
                            ItemResult::Error {
                                source: source_desc,
                                error: e.to_string(),
                            }
                        } else {
                            pb.inc(1);
                            ItemResult::Fatal(e)
                        }
                    }
                }
            }
        })
        .buffer_unordered(concurrency_limit);

    // Process results as they complete - no need to hold all results in memory
    while let Some(result) = result_stream.next().await {
        match result {
            ItemResult::Success {
                metadata,
                item_href,
                title,
            } => {
                accumulator.add_item(metadata, item_href, title);
                if memory_logging_enabled()
                    && accumulator
                        .successful_count()
                        .is_multiple_of(memory_log_every)
                {
                    log_memory(format!(
                        "collection-progress processed={} errors={}",
                        accumulator.successful_count(),
                        accumulator.error_count()
                    ));
                }
            }
            ItemResult::Error { source, error } => {
                accumulator.add_error(source, error);
                if memory_logging_enabled()
                    && accumulator.error_count().is_multiple_of(memory_log_every)
                {
                    log_memory(format!(
                        "collection-errors processed={} errors={}",
                        accumulator.successful_count(),
                        accumulator.error_count()
                    ));
                }
            }
            ItemResult::Fatal(e) => {
                return Err(e);
            }
        }
    }
    pb_arc.finish_and_clear();
    log_memory(format!(
        "collection-items-finished processed={} errors={}",
        accumulator.successful_count(),
        accumulator.error_count()
    ));

    // Check if collection file exists and overwrite flag
    let collection_path = config.output.join("collection.json");
    if collection_path.exists() && !config.overwrite_collection {
        print_warning(
            "Collection file already exists, skipping (use --overwrite-collection to regenerate)",
        );

        // Note: parent/root links are refreshed at the catalog level after all
        // per-collection processing completes — see handle_catalog_command.

        // Still generate GeoParquet if requested.
        // Skip in config-only mode: no items dir exists and there's nothing to write.
        if config.geoparquet && !config_only {
            let mut items_for_parquet: Vec<crate::stac::StacItem> = Vec::new();
            let spinner = create_spinner("Reading existing items for GeoParquet…");
            for entry in std::fs::read_dir(&items_dir)? {
                let entry = entry?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(item) = serde_json::from_str::<crate::stac::StacItem>(&content) {
                            items_for_parquet.push(item);
                        }
                    }
                }
            }
            finish_spinner_ok(
                spinner,
                format!("Read {} item(s) from disk", items_for_parquet.len()),
            );

            if !items_for_parquet.is_empty() {
                // Read existing collection, add geoparquet asset, write back
                let collection_content = std::fs::read_to_string(&collection_path)?;
                let mut collection: crate::stac::StacCollection =
                    serde_json::from_str(&collection_content)?;

                // Add items-geoparquet asset if not already present
                collection
                    .assets
                    .entry("items-geoparquet".to_string())
                    .or_insert_with(make_geoparquet_asset);

                // Write updated collection back
                let updated_json = if config.pretty {
                    serde_json::to_string_pretty(&collection)?
                } else {
                    serde_json::to_string(&collection)?
                };
                std::fs::write(&collection_path, &updated_json)?;

                // Write parquet file
                let parquet_path = config.output.join("items.parquet");
                let spinner = create_spinner("Writing GeoParquet…");
                crate::stac::geoparquet::write_geoparquet(
                    &items_for_parquet,
                    &collection,
                    &parquet_path,
                )?;
                finish_spinner_ok(
                    spinner,
                    format!(
                        "GeoParquet written: {} ({} items)",
                        parquet_path.display(),
                        items_for_parquet.len()
                    ),
                );
            }
        }

        // Return info about existing collection
        return Ok((collection_path, collection_id, merged_config.title));
    }

    // Check for errors - only generate collection if no errors
    if accumulator.has_errors() {
        print_error(format!(
            "Collection generation failed: {} item(s) had errors",
            accumulator.error_count()
        ));

        // Print details about errors
        for (source, error) in &accumulator.errors {
            eprintln!("  {} {}: {}", console::style("✗").red(), source, error);
        }

        return Err(CityJsonStacError::StacError(format!(
            "{} item(s) failed to process",
            accumulator.error_count()
        )));
    }

    // Build collection from accumulated metadata
    let license = merged_config
        .license
        .clone()
        .unwrap_or_else(|| config.license.clone());

    let mut collection_builder = StacCollectionBuilder::new(&collection_id).license(license);

    // Set temporal extent from config or default
    if let Some(temporal) = merged_config
        .extent
        .as_ref()
        .and_then(|e| e.temporal.as_ref())
    {
        let start = temporal
            .start
            .as_ref()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok());
        let end = temporal
            .end
            .as_ref()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok());
        collection_builder = collection_builder.temporal_extent(start, end);
    } else {
        collection_builder = collection_builder.temporal_extent(Some(chrono::Utc::now()), None);
    }

    if !config_only {
        // Normal mode: aggregate metadata from processed items
        collection_builder = collection_builder.aggregate_from_summaries(&accumulator.summaries)?;
    } else {
        // Config-only mode: use bbox from config extent
        if let Some(bbox) = merged_config
            .extent
            .as_ref()
            .and_then(|e| e.spatial.as_ref())
            .and_then(|s| s.bbox.as_ref())
        {
            let bbox3d = if bbox.len() == 6 {
                crate::metadata::BBox3D::new(bbox[0], bbox[1], bbox[2], bbox[3], bbox[4], bbox[5])
            } else if bbox.len() >= 4 {
                crate::metadata::BBox3D::new(bbox[0], bbox[1], 0.0, bbox[2], bbox[3], 0.0)
            } else {
                return Err(CityJsonStacError::StacError(
                    "Config bbox must have 4 or 6 elements".to_string(),
                ));
            };

            // STAC requires the collection bbox to be in WGS84 (per RFC 7946).
            // If the values are already in WGS84 valid range, treat them as WGS84 —
            // real projected coordinates (UTM eastings, Lambert false-easting offsets,
            // state-plane feet, etc.) are far outside [-180, 180] x [-90, 90], so a
            // bbox in that range is almost certainly already in WGS84 even when
            // `extent.spatial.crs` names a projected native CRS for items.
            let in_wgs84_range = bbox3d.xmin >= -180.0
                && bbox3d.xmax <= 180.0
                && bbox3d.ymin >= -90.0
                && bbox3d.ymax <= 90.0;
            let wgs84_bbox = if in_wgs84_range {
                bbox3d
            } else {
                let crs = crs_override.clone().unwrap_or_default();
                bbox3d.to_wgs84(&crs)?
            };
            collection_builder = collection_builder.spatial_extent(wgs84_bbox);
        }
    }

    // Apply config-based metadata
    if let Some(t) = &merged_config.title {
        collection_builder = collection_builder.title(t.clone());
    }

    if let Some(d) = &merged_config.description {
        collection_builder = collection_builder.description(d.clone());
    }

    if let Some(keywords) = &merged_config.keywords {
        collection_builder = collection_builder.keywords(keywords.clone());
    }

    if let Some(providers) = &merged_config.providers {
        for provider in providers {
            collection_builder = collection_builder.provider(provider.clone().into());
        }
    }

    // Apply config summaries, unioned with any auto-detected values for the same key
    if let Some(summaries) = &merged_config.summaries {
        for (key, value) in summaries {
            collection_builder = collection_builder.summary_union(key.clone(), value.clone());
        }
    }

    // Apply config links
    if let Some(links) = &merged_config.links {
        for link_cfg in links {
            let mut link = stac::Link::new(&link_cfg.href, &link_cfg.rel);
            link.r#type = link_cfg.link_type.clone();
            link.title = link_cfg.title.clone();
            collection_builder = collection_builder.link(link);
        }
    }

    // Apply config assets
    if let Some(assets) = &merged_config.assets {
        for (key, asset_cfg) in assets {
            let mut asset = stac::Asset::new(&asset_cfg.href);
            asset.r#type = asset_cfg.media_type.clone();
            asset.title = asset_cfg.title.clone();
            asset.description = asset_cfg.description.clone();
            if let Some(roles) = &asset_cfg.roles {
                asset.roles = roles.clone();
            }
            collection_builder = collection_builder.asset(key.clone(), asset);
        }
    }

    // Add item links from accumulator
    for (href, title) in &accumulator.item_links {
        collection_builder = collection_builder.item_link(href.clone(), title.clone());
    }
    if accumulator.omitted_item_links() > 0 {
        print_warning(format!(
            "Omitted {} item link(s) from collection.json due to --max-item-links limit",
            accumulator.omitted_item_links()
        ));
    }

    // Add self link
    collection_builder = collection_builder.self_link("./collection.json");

    // Add parent and root links (set when collection is part of a catalog)
    if let Some(parent_href) = &config.parent_href {
        collection_builder = collection_builder.parent_link(parent_href);
    }
    if let Some(root_href) = &config.root_href {
        collection_builder = collection_builder.root_link(root_href);
    }

    // Add GeoParquet asset marker if enabled (actual write happens after collection is built).
    // Skip in config-only mode: no items means no parquet file will be written.
    if config.geoparquet && !config_only {
        collection_builder = collection_builder.asset("items-geoparquet", make_geoparquet_asset());
    }

    // Build and write collection
    let collection = collection_builder.build()?;
    let collection_json = if config.pretty {
        serde_json::to_string_pretty(&collection)?
    } else {
        serde_json::to_string(&collection)?
    };
    log_memory("collection-before-write-json");

    std::fs::write(&collection_path, &collection_json)?;
    log_memory("collection-after-write-json");

    // Write GeoParquet file if enabled — read items from disk to avoid holding them all in memory.
    // Skip in config-only mode: there are no items, and the items dir is never created.
    let mut geoparquet_item_count = 0;
    if config.geoparquet && !config_only {
        log_memory("geoparquet-read-start");
        let spinner = create_spinner("Reading items from disk for GeoParquet…");
        let mut geoparquet_items: Vec<crate::stac::StacItem> = Vec::new();
        for entry in std::fs::read_dir(&items_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(item) = serde_json::from_str::<crate::stac::StacItem>(&content) {
                        geoparquet_items.push(item);
                    }
                }
            }
        }
        finish_spinner_ok(
            spinner,
            format!("Read {} item(s) from disk", geoparquet_items.len()),
        );

        if !geoparquet_items.is_empty() {
            geoparquet_item_count = geoparquet_items.len();
            let parquet_path = config.output.join("items.parquet");
            let spinner = create_spinner("Writing GeoParquet…");
            log_memory(format!(
                "geoparquet-write-start items={}",
                geoparquet_items.len()
            ));
            crate::stac::geoparquet::write_geoparquet(
                &geoparquet_items,
                &collection,
                &parquet_path,
            )?;
            log_memory("geoparquet-write-finished");
            finish_spinner_ok(
                spinner,
                format!(
                    "GeoParquet written: {} ({} items)",
                    parquet_path.display(),
                    geoparquet_items.len()
                ),
            );
        }
    }

    // Print summary
    let mut summary = Summary::new()
        .add("Collection", collection_path.display().to_string())
        .add("Items dir", items_dir.display().to_string())
        .add(
            "Items generated",
            format!("{}", accumulator.successful_count()),
        );
    if accumulator.omitted_item_links() > 0 {
        summary = summary.add(
            "Item links omitted",
            format!("{}", accumulator.omitted_item_links()),
        );
    }
    if config.geoparquet && geoparquet_item_count > 0 {
        summary = summary.add(
            "GeoParquet",
            config.output.join("items.parquet").display().to_string(),
        );
    }
    summary.print();

    print_success("Collection generated successfully");

    Ok((collection_path, collection_id, merged_config.title))
}

/// Configuration for update-collection/aggregate command
struct UpdateCollectionConfig {
    items: Vec<PathBuf>,
    output: PathBuf,
    config: Option<PathBuf>,
    id: Option<String>,
    title: Option<String>,
    description: Option<String>,
    license: String,
    items_base_url: Option<String>,
    skip_errors: bool,
    pretty: bool,
    dry_run: bool,
    geoparquet: bool,
    max_item_links: Option<usize>,
}

fn handle_update_collection_command(config: UpdateCollectionConfig) -> Result<()> {
    // Dry-run mode: validate only
    if config.dry_run {
        use progress::{print_banner, print_error, print_success};

        print_banner();

        println!("\nRunning in dry-run mode...\n");

        let mut all_valid = true;
        let mut found = 0;

        for item_path in &config.items {
            let fname = item_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");

            if item_path.exists() {
                // Try to parse as STAC item
                match std::fs::read_to_string(item_path) {
                    Ok(content) => match serde_json::from_str::<crate::stac::StacItem>(&content) {
                        Ok(_) => {
                            println!("  ✓ {}", fname);
                            found += 1;
                        }
                        Err(e) => {
                            println!("  ✗ {}: Invalid STAC item - {}", fname, e);
                            all_valid = false;
                        }
                    },
                    Err(e) => {
                        println!("  ✗ {}: Cannot read - {}", fname, e);
                        all_valid = false;
                    }
                }
            } else {
                println!("  ✗ {}: File not found", fname);
                all_valid = false;
            }
        }

        println!("\n  STAC items: {}/{} valid", found, config.items.len());

        println!();

        if all_valid {
            print_success("Dry run complete: All validations passed");
            std::process::exit(0);
        } else {
            print_error("Dry run failed: Errors found");
            std::process::exit(1);
        }
    }

    // Load config file if provided
    let base_config = if let Some(config_path) = &config.config {
        CollectionConfigFile::from_file(config_path)?
    } else {
        CollectionConfigFile::default()
    };

    // Merge with CLI args
    let merged_config = base_config.merge_with_cli(&CollectionCliArgs {
        id: config.id.clone(),
        title: config.title.clone(),
        description: config.description.clone(),
        license: if config.license != "proprietary" {
            Some(config.license.clone())
        } else {
            None
        },
        base_url: None, // update-collection uses items_base_url for item links, not asset hrefs
        concurrency: None,
    });

    log::info!(
        "Aggregating {} STAC items into collection",
        config.items.len()
    );

    if config.items.is_empty() {
        return Err(crate::error::CityJsonStacError::StacError(
            "No STAC item files provided".to_string(),
        ));
    }

    // Parse all STAC items
    let mut parsed_items: Vec<crate::stac::StacItem> = Vec::new();
    let mut errors: Vec<(PathBuf, String)> = Vec::new();

    let pb = create_progress_bar(config.items.len() as u64, "Parsing STAC items…");
    for item_path in &config.items {
        let fname = item_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        pb.set_message(format!("Parsing: {fname}"));
        match std::fs::read_to_string(item_path) {
            Ok(content) => match serde_json::from_str::<crate::stac::StacItem>(&content) {
                Ok(item) => {
                    parsed_items.push(item);
                }
                Err(e) => {
                    if config.skip_errors {
                        errors.push((item_path.clone(), e.to_string()));
                        pb.println(format!(
                            "  {} Skipping {fname}: {e}",
                            console::style("⚠").yellow()
                        ));
                    } else {
                        pb.finish_and_clear();
                        return Err(crate::error::CityJsonStacError::JsonError(e));
                    }
                }
            },
            Err(e) => {
                if config.skip_errors {
                    errors.push((item_path.clone(), e.to_string()));
                    pb.println(format!(
                        "  {} Skipping {fname}: {e}",
                        console::style("⚠").yellow()
                    ));
                } else {
                    pb.finish_and_clear();
                    return Err(crate::error::CityJsonStacError::IoError(e));
                }
            }
        }
        pb.inc(1);
    }
    pb.finish_and_clear();

    if parsed_items.is_empty() {
        return Err(crate::error::CityJsonStacError::StacError(
            "No valid STAC items could be parsed".to_string(),
        ));
    }

    // Generate collection ID from first item or output filename
    let collection_id = merged_config.id.unwrap_or_else(|| {
        config
            .output
            .file_stem()
            .and_then(|n| n.to_str())
            .unwrap_or("collection")
            .to_string()
    });

    let license = merged_config
        .license
        .unwrap_or_else(|| config.license.clone());

    // Build collection by aggregating item metadata
    let mut collection_builder = StacCollectionBuilder::new(&collection_id)
        .license(license)
        .temporal_extent(Some(chrono::Utc::now()), None)
        .aggregate_from_items(&parsed_items)?;

    // Apply config-based metadata
    if let Some(t) = merged_config.title {
        collection_builder = collection_builder.title(t);
    }

    if let Some(d) = merged_config.description {
        collection_builder = collection_builder.description(d);
    }

    if let Some(keywords) = merged_config.keywords {
        collection_builder = collection_builder.keywords(keywords);
    }

    if let Some(providers) = merged_config.providers {
        for provider in providers {
            collection_builder = collection_builder.provider(provider.into());
        }
    }

    // Override aggregated extent with config-provided values when present.
    // aggregate_from_items defaults temporal to Utc::now() when items lack a
    // datetime and can produce a mixed-CRS spatial bbox if some items predate
    // the bbox-CRS fix; the config extent is authoritative for the collection.
    if let Some(temporal) = merged_config
        .extent
        .as_ref()
        .and_then(|e| e.temporal.as_ref())
    {
        let start = temporal
            .start
            .as_ref()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok());
        let end = temporal
            .end
            .as_ref()
            .and_then(|s| s.parse::<chrono::DateTime<chrono::Utc>>().ok());
        collection_builder = collection_builder.temporal_extent(start, end);
    }

    if let Some(bbox) = merged_config
        .extent
        .as_ref()
        .and_then(|e| e.spatial.as_ref())
        .and_then(|s| s.bbox.as_ref())
    {
        let bbox3d = if bbox.len() == 6 {
            crate::metadata::BBox3D::new(bbox[0], bbox[1], bbox[2], bbox[3], bbox[4], bbox[5])
        } else if bbox.len() >= 4 {
            crate::metadata::BBox3D::new(bbox[0], bbox[1], 0.0, bbox[2], bbox[3], 0.0)
        } else {
            return Err(crate::error::CityJsonStacError::StacError(
                "Config bbox must have 4 or 6 elements".to_string(),
            ));
        };
        // Same WGS84-range heuristic as the collection command: a bbox already
        // inside [-180,180] x [-90,90] is treated as WGS84 even when
        // extent.spatial.crs names a projected native CRS for the items.
        let in_wgs84_range = bbox3d.xmin >= -180.0
            && bbox3d.xmax <= 180.0
            && bbox3d.ymin >= -90.0
            && bbox3d.ymax <= 90.0;
        let wgs84_bbox = if in_wgs84_range {
            bbox3d
        } else {
            let crs = merged_config
                .extent
                .as_ref()
                .and_then(|e| e.spatial.as_ref())
                .and_then(|s| s.crs.as_deref())
                .and_then(CRS::from_citygml_srs_name)
                .unwrap_or_default();
            bbox3d.to_wgs84(&crs)?
        };
        collection_builder = collection_builder.spatial_extent(wgs84_bbox);
    }

    // Apply curated dataset-level links from config (about/source/license/related).
    if let Some(links) = &merged_config.links {
        for link_cfg in links {
            let mut link = stac::Link::new(&link_cfg.href, &link_cfg.rel);
            link.r#type = link_cfg.link_type.clone();
            link.title = link_cfg.title.clone();
            collection_builder = collection_builder.link(link);
        }
    }

    // Apply config-defined summaries, unioned with any auto-aggregated ones.
    if let Some(summaries) = &merged_config.summaries {
        for (key, value) in summaries {
            collection_builder = collection_builder.summary_union(key.clone(), value.clone());
        }
    }

    // Apply config-defined collection-level assets.
    if let Some(assets) = &merged_config.assets {
        for (key, asset_cfg) in assets {
            let mut asset = stac::Asset::new(&asset_cfg.href);
            asset.r#type = asset_cfg.media_type.clone();
            asset.title = asset_cfg.title.clone();
            asset.description = asset_cfg.description.clone();
            if let Some(roles) = &asset_cfg.roles {
                asset.roles = roles.clone();
            }
            collection_builder = collection_builder.asset(key.clone(), asset);
        }
    }

    // Add item links
    let max_item_links = config.max_item_links.unwrap_or(usize::MAX);
    let mut omitted_item_links = 0usize;
    for (idx, (item_path, item)) in config.items.iter().zip(parsed_items.iter()).enumerate() {
        if idx >= max_item_links {
            omitted_item_links += 1;
            continue;
        }
        let fallback_filename = format!("{}.json", item.id);
        let item_filename = item_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&fallback_filename);

        let href = match &config.items_base_url {
            Some(base) => {
                // Ensure base URL ends with a slash
                let normalized_base = if base.ends_with('/') {
                    base.to_string()
                } else {
                    format!("{base}/")
                };
                format!("{normalized_base}{item_filename}")
            }
            None => {
                // Use relative path from collection to item
                format!("./{item_filename}")
            }
        };

        collection_builder = collection_builder.item_link(href, Some(item.id.clone()));
    }
    if omitted_item_links > 0 {
        print_warning(format!(
            "Omitted {} item link(s) from collection.json due to --max-item-links limit",
            omitted_item_links
        ));
    }

    // Add self link
    collection_builder = collection_builder.self_link("./collection.json");

    // Add GeoParquet asset if enabled
    if config.geoparquet && !parsed_items.is_empty() {
        collection_builder = collection_builder.asset("items-geoparquet", make_geoparquet_asset());
    }

    // Build and write collection
    let collection = collection_builder.build()?;
    let collection_json = if config.pretty {
        serde_json::to_string_pretty(&collection)?
    } else {
        serde_json::to_string(&collection)?
    };

    // Create parent directory if needed
    if let Some(parent) = config.output.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(&config.output, &collection_json)?;

    // Write GeoParquet file if enabled
    if config.geoparquet && !parsed_items.is_empty() {
        let parquet_path = config
            .output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("items.parquet");
        let spinner = create_spinner("Writing GeoParquet…");
        crate::stac::geoparquet::write_geoparquet(&parsed_items, &collection, &parquet_path)?;
        finish_spinner_ok(
            spinner,
            format!(
                "GeoParquet written: {} ({} items)",
                parquet_path.display(),
                parsed_items.len()
            ),
        );
    }

    // Print summary
    let mut summary = Summary::new()
        .add("Collection", config.output.display().to_string())
        .add("Items aggregated", format!("{}", parsed_items.len()));
    if omitted_item_links > 0 {
        summary = summary.add("Item links omitted", format!("{omitted_item_links}"));
    }
    if !errors.is_empty() {
        summary = summary.add("Skipped", format!("{} item(s)", errors.len()));
    }
    if config.geoparquet && !parsed_items.is_empty() {
        let parquet_path = config
            .output
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("items.parquet");
        summary = summary.add("GeoParquet", parquet_path.display().to_string());
    }
    summary.print();

    if errors.is_empty() {
        print_success("Collection updated successfully");
    } else {
        print_warning(format!(
            "Collection updated with {} skipped item(s)",
            errors.len()
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_folder_name_basic() {
        // Valid characters should pass through
        assert_eq!(sanitize_folder_name("my-collection"), "my-collection");
        assert_eq!(sanitize_folder_name("my_collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my.collection"), "my.collection");
        assert_eq!(sanitize_folder_name("collection123"), "collection123");
    }

    #[test]
    fn test_sanitize_folder_name_spaces() {
        // Spaces should be replaced with underscores
        assert_eq!(sanitize_folder_name("my collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my  collection"), "my__collection");
    }

    #[test]
    fn test_sanitize_folder_name_special_chars() {
        // Special characters should be replaced with underscores
        assert_eq!(sanitize_folder_name("my@collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my/collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my\\collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my:collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my*collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my?collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my<collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my>collection"), "my_collection");
        assert_eq!(sanitize_folder_name("my|collection"), "my_collection");
    }

    #[test]
    fn test_sanitize_folder_name_unicode() {
        // Unicode letters are alphanumeric and pass through (good for internationalization)
        assert_eq!(sanitize_folder_name("münchen"), "münchen");
        assert_eq!(sanitize_folder_name("東京"), "東京");
        // But special unicode symbols are replaced
        assert_eq!(sanitize_folder_name("hello★world"), "hello_world");
    }

    #[test]
    fn test_sanitize_folder_name_mixed() {
        // Mixed valid and invalid characters
        assert_eq!(
            sanitize_folder_name("my awesome collection!"),
            "my_awesome_collection_"
        );
        assert_eq!(
            sanitize_folder_name("collection (v1.0)"),
            "collection__v1.0_"
        );
    }

    #[test]
    fn test_fallback_folder_name() {
        assert_eq!(fallback_folder_name("path/to/config.yaml"), "config.yaml");
        assert_eq!(
            fallback_folder_name("./opendata/vienna-config.yaml"),
            "vienna-config.yaml"
        );
        assert_eq!(fallback_folder_name("config"), "config");
    }
}
