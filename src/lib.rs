
use yaffe_plugin::*;
use std::collections::HashMap;

#[no_mangle]
pub fn initialize() -> Box<dyn YaffePlugin> {
    Box::new(YaffePlex {
        client: None,
        path: std::path::PathBuf::new(),
        server_base_path: String::from(""),
        token: String::from(""),
    })
}

struct YaffePlex {
    client: Option<reqwest::blocking::Client>,
    path: std::path::PathBuf,
    server_base_path: String,
    token: String,
}

impl YaffePlex {
    fn client(&self) -> &reqwest::blocking::Client {
        self.client.as_ref().unwrap()
    }

    fn get_plex_path(&self, path: &str) -> String {
        format!("{}{}?X-Plex-Token={}", self.server_base_path, path, self.token)
    }
}

impl YaffePlugin for YaffePlex {
    fn name(&self) -> &'static str {
        "Plex"
    }

    fn initialize(&mut self, settings: &HashMap<String, PluginSetting>) -> InitializeResult {
        self.client = Some(reqwest::blocking::Client::new());
        self.server_base_path = match try_get_str(settings, "plex_server") {
            None => return Err(String::from("plex_server setting is required")),
            Some(s) => s,
        };

        self.token = match try_get_str(settings, "plex_token") {
            None => return Err(String::from("plex_token setting is required")),
            Some(s) => s,
        };

        Ok(())
    }

    fn settings(&self) -> Vec<(&'static str, PluginSetting)> {
        let mut result = vec!();
        result.push(("plex_server", PluginSetting::String("".to_string())));
        result.push(("plex_token", PluginSetting::String("".to_string())));
        result
    }

    fn initial_load(&mut self) {
        self.path = std::path::PathBuf::new();
        self.path.push("/library/sections");
    }

    fn load_items(&mut self, _: u32, _: &HashMap<String, PluginSetting>) -> LoadResult {
        let mut result = vec!();
        if self.server_base_path.is_empty() || self.token.is_empty() {
            return Err(String::from("plex_server or plex_token settings missing"));
        }

        let mut server = self.server_base_path.trim_end_matches('/').to_string();

        server.push_str(&self.path.to_string_lossy());
        let resp = self.client().get(server.clone())
                                .query(&[("X-Plex-Token", self.token.clone())])
                                .send().map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            let text = &resp.text().unwrap();
            let doc = roxmltree::Document::parse(text).unwrap();

            for d in doc.descendants() {


                match d.tag_name().name() {
                    "Directory" => {
                        let (name, art_path) = get_title_and_art(&self, d);
                        let path = d.attribute("key").unwrap();
                        result.push(YaffePluginItem::new(String::from(name), 
                                                        String::from(path), 
                                                        art_path, 
                                                        false, 
                                                        String::from("")));
                    },
                    "Video" => {
                        let path = match d.descendants().find(|v| v.has_tag_name("Part")) {
                            Some(path) => {
                                path.attribute("key").unwrap()
                            },
                            None => "",
                        };
                        let (name, art_path) = get_title_and_art(&self, d);
                        let description = d.attribute("summary").unwrap();
                        let rating = d.attribute("contentRating").unwrap_or_default();

                        result.push(YaffePluginItem::new(String::from(name), 
                                                        String::from(path), 
                                                        art_path, 
                                                        rating == "R", 
                                                        String::from(description)));
                    },
                    _ => {},
                }
                
            }

        } else {
            return Err(format!("returned status {}", resp.status()))
        }
        finish_loading(result)
    }
    
    fn on_selected(&mut self, _name: &str, path: &str, _: &HashMap<String, PluginSetting>) -> SelectedAction {
        if path.starts_with("/library/parts") {
            #[cfg(windows)]
            let mut command = std::process::Command::new("explorer");
            #[cfg(not(windows))]
            let mut command = std::process::Command::new("open");

            command.args(&[self.get_plex_path(path)]);

            SelectedAction::Start(command)
        } else {
            if path.starts_with('/') {
                self.path = std::path::PathBuf::new();
            }
            self.path.push(path);
            SelectedAction::Load
        }
    }

    fn on_back(&mut self) -> bool {
        self.path.pop();
        true
    }
}

fn get_title_and_art<'a>(plex: &YaffePlex, node: roxmltree::Node<'a, 'a>) -> (&'a str, PathType) {
    let art_path = match node.attribute("art") {
        Some(art) => PathType::Url(plex.get_plex_path(art)),
        None => PathType::File(String::from("./folder.jpg")),
    };
    (node.attribute("title").unwrap(), art_path)
}