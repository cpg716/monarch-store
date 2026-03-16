fn main() { let col = appstream::Collection::from_path("/var/lib/flatpak/appstream/flathub/x86_64/active/appstream.xml.gz".into()); println!("Count: {}", col.unwrap().components.len()); }
