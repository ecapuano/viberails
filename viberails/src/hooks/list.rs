use anyhow::Result;

use crate::{common::print_header, providers::ProviderRegistry};

////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////

pub fn list() -> Result<()> {
    let registry = ProviderRegistry::new();

    print_header();

    for factory in registry.all() {
        let discovery = factory.discover();

        println!("\nProvider: {} ({})", factory.display_name(), factory.id());

        if !discovery.detected {
            println!("    [not detected]");
            if let Some(hint) = discovery.detection_hint {
                println!("    Hint: {hint}");
            }
            continue;
        }

        match factory.create() {
            Ok(provider) => match provider.list() {
                Ok(hooks) => {
                    if hooks.is_empty() {
                        println!("    [no hooks installed]");
                    } else {
                        for hook in hooks {
                            println!(
                                "    {:<20} {:<10} {}",
                                hook.hook_type, hook.matcher, hook.command
                            );
                        }
                    }
                }
                Err(e) => {
                    println!("    [error listing hooks: {e}]");
                }
            },
            Err(e) => {
                println!("    [error creating provider: {e}]");
            }
        }
    }

    Ok(())
}
