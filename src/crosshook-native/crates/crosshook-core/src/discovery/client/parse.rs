use super::{DiscoveryError, RssItem};

pub(super) fn parse_rss_items(xml: &str) -> Result<Vec<RssItem>, DiscoveryError> {
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    let mut reader = Reader::from_str(xml);
    let mut items = Vec::new();
    let mut in_item = false;
    let mut current_tag = String::new();
    let mut title = String::new();
    let mut link = String::new();
    let mut pub_date = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "item" {
                    in_item = true;
                    title.clear();
                    link.clear();
                    pub_date.clear();
                } else if in_item {
                    current_tag = tag_name;
                }
            }
            Ok(Event::Text(ref e)) if in_item => {
                let text = e.decode().unwrap_or_default().to_string();
                match current_tag.as_str() {
                    "title" => title.push_str(&text),
                    "link" => link.push_str(&text),
                    "pubDate" => pub_date.push_str(&text),
                    _ => {}
                }
            }
            Ok(Event::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag_name == "item" && in_item {
                    if !title.is_empty() && !link.is_empty() {
                        items.push(RssItem {
                            title: title.trim().to_string(),
                            link: link.trim().to_string(),
                            pub_date: if pub_date.trim().is_empty() {
                                None
                            } else {
                                Some(pub_date.trim().to_string())
                            },
                        });
                    }
                    in_item = false;
                }
                current_tag.clear();
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(DiscoveryError::ParseError(format!(
                    "XML parse error at position {}: {error}",
                    reader.error_position()
                )));
            }
            _ => {}
        }
    }

    Ok(items)
}
