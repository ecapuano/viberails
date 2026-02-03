use crate::{
    common::print_header,
    providers::ProviderRegistry,
    tui::{ConfigEntry, ConfigView},
};

////////////////////////////////////////////////////////////////////////////////
////////////////////////////////////////////////////////////////////////////////

pub fn list() {
    let registry = ProviderRegistry::new();

    print_header();

    for factory in registry.all() {
        let discovery = factory.discover();
        let title = format!(" {} ({}) ", factory.display_name(), factory.id());

        if !discovery.detected {
            let mut entries = vec![ConfigEntry::new("Status", "not detected")];
            if let Some(hint) = discovery.detection_hint {
                entries.push(ConfigEntry::new("Hint", hint));
            }
            ConfigView::new(&title, entries).print();
            continue;
        }

        match factory.create() {
            Ok(provider) => match provider.list() {
                Ok(hooks) => {
                    if hooks.is_empty() {
                        let entries = vec![ConfigEntry::new("Status", "no hooks installed")];
                        ConfigView::new(&title, entries).print();
                    } else {
                        let entries: Vec<ConfigEntry> = hooks
                            .iter()
                            .map(|hook| {
                                ConfigEntry::new(
                                    &hook.hook_type,
                                    format!("{} {}", hook.matcher, hook.command),
                                )
                            })
                            .collect();
                        ConfigView::new(&title, entries).print();
                    }
                }
                Err(e) => {
                    let entries = vec![ConfigEntry::new("Error", format!("listing hooks: {e}"))];
                    ConfigView::new(&title, entries).print();
                }
            },
            Err(e) => {
                let entries = vec![ConfigEntry::new("Error", format!("creating provider: {e}"))];
                ConfigView::new(&title, entries).print();
            }
        }
    }
}
