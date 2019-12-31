use once_cell::sync::OnceCell;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

pub trait Template {
    fn parent() -> &'static str;
    fn name() -> &'static str;
    fn variant(&self) -> &'static str;
    fn apply(&self, input: &str) -> Option<String>;
}

pub struct TemplateResolver;

impl TemplateResolver {
    pub fn load(parent: &str, name: &str, template_file: impl AsRef<Path>) -> Option<String> {
        STORE
            .get_or_init(|| Mutex::new(Templates::new(template_file.as_ref().to_owned())))
            .lock()
            .unwrap()
            .refresh_and_get(parent)?
            .get(name)
            .cloned()
    }
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct Pair<T: Hash + Eq + Sized, V = T>(HashMap<T, V>);

impl<T: Hash + Eq> Pair<T> {
    pub fn get<K: ?Sized>(&self, key: &K) -> Option<&T>
    where
        K: Hash + Eq + std::fmt::Display,
        T: Borrow<K>,
    {
        self.0.get(key)
    }
}

pub type TemplateMap<T> = HashMap<T, Pair<T>>;

#[derive(Debug, serde::Deserialize)]
pub struct Templates {
    #[serde(skip)]
    start: Option<SystemTime>,
    #[serde(skip)]
    template_file: PathBuf,
    templates: TemplateMap<String>,
}

impl Templates {
    pub fn new(template_file: impl Into<PathBuf>) -> Self {
        let mut this = Self {
            start: SystemTime::now().into(),
            template_file: template_file.into(),
            templates: Default::default(),
        };
        this.refresh();
        this
    }

    pub fn refresh_and_get<K: ?Sized>(&mut self, parent: &K) -> Option<&Pair<String>>
    where
        K: Hash + Eq + std::fmt::Display,
        String: Borrow<K>,
    {
        self.refresh();
        self.templates.get(parent)
    }

    fn refresh(&mut self) {
        let mtime = match std::fs::metadata(&self.template_file).and_then(|md| md.modified()) {
            Ok(mtime) => mtime,
            Err(err) => {
                log::error!(
                    "cannot read template ({}) file: {}",
                    self.template_file.display(),
                    err
                );
                return;
            }
        };

        let start = match self.start {
            Some(start) => start,
            None => {
                log::error!("template state is fatally invalid. please restart the bot");
                return;
            }
        };

        if start < mtime || self.templates.is_empty() {
            match std::fs::read_to_string(&self.template_file)
                .ok()
                .and_then(|data| toml::from_str::<TemplateMap<String>>(&data).ok())
            {
                Some(map) => {
                    log::info!("reloaded templates");
                    self.start.replace(mtime);
                    std::mem::replace(&mut self.templates, map);
                }
                None => log::info!(
                    "cannot read templates from '{}'. not updating them",
                    self.template_file.display()
                ),
            }
        }
    }
}

static STORE: OnceCell<Mutex<Templates>> = OnceCell::new();
