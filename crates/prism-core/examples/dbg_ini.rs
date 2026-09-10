use prism_core::ini::{parse_qt, save_ini, IniMap};
use std::path::Path;

fn show(label: &str, text: &str) {
    let m = parse_qt(text, Path::new("t")).unwrap();
    println!("{label}: {:?}", m.entries());
}

fn main() {
    show("one-line", "NAME=Beta\n");
    show("two-lines", "name=Alpha\nNAME=Beta\n");
    show("no-trailing-nl", "name=Alpha\nNAME=Beta");
    show("crlf", "name=Alpha\r\nNAME=Beta\r\n");
    show("basic", "name=Alpha\nNAME=Beta\niconKey=default\n");

    let mut map = IniMap::new();
    map.set("k1", "quote\"inside");
    map.set("k2", "back\\slash");
    let t = save_ini(&map);
    println!("exotic saved: {t:?}");
    let back = parse_qt(&t, Path::new("t")).unwrap();
    println!("exotic back: {:?}", back.entries());
}
