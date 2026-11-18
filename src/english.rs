//! 场次字段的本机英文词典. 只改能对上的专名, 其余留作核对.

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionChange {
    pub key: String,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionNote {
    pub key: String,
    pub note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionNormalize {
    pub changes: Vec<SessionChange>,
    pub notes: Vec<SessionNote>,
    pub verified: Vec<String>,
}

pub fn field_label(key: &str) -> &str {
    match key {
        "name" => "场次 / 专辑名",
        "date" => "演出日期",
        "performer" => "演奏家",
        "conductor" => "指挥家",
        "composer" => "作曲家",
        "orchestra" => "乐团",
        "city" => "城市",
        "venue" => "音乐厅",
        "origin" => "录音来源",
        _ => key,
    }
}

pub fn normalize(fields: &[(&str, &str)]) -> SessionNormalize {
    let value = |key: &str| fields.iter().find(|(name, _)| *name == key).map(|(_, text)| clean(text)).unwrap_or_default();
    let hall = location_for(&value("venue"));
    let city = dict_get(CITIES, &value("city"));
    let mismatch = hall.is_some_and(|item| !value("city").is_empty() && lookup_key(city.unwrap_or(value("city").as_str())) != lookup_key(item.city));
    let mut changes = Vec::new();
    let mut notes = Vec::new();
    let mut verified = Vec::new();
    for (key, raw) in fields {
        let input = clean(raw);
        if input.is_empty() {
            continue;
        }
        if *key == "date" {
            if valid_date(&input) {
                verified.push((*key).to_string());
            } else {
                notes.push(SessionNote { key: (*key).to_string(), note: "请核对日期, 格式为 YYYY-MM-DD.".into() });
            }
            continue;
        }
        if mismatch && matches!(*key, "city" | "venue") {
            let city_name = hall.map(|item| item.city).unwrap_or("");
            notes.push(SessionNote { key: (*key).to_string(), note: format!("场馆目录对应 {city_name}, 请核对城市与音乐厅.") });
            continue;
        }
        let resolved = match *key {
            "name" => session_title(&input, &value),
            "city" => city.unwrap_or("").to_string(),
            "venue" => hall.and_then(|item| dict_get(item.venues, &input)).unwrap_or("").to_string(),
            "orchestra" => normalize_list(&input, |name| dict_get(ORCHESTRAS, name)),
            "origin" => dict_get(ORIGINS, &input).unwrap_or("").to_string(),
            "performer" | "conductor" => normalize_list(&input, |name| dict_get(PEOPLE, name)),
            "composer" => normalize_list(&input, |name| composer_name(name)),
            _ => String::new(),
        };
        if !resolved.is_empty() && resolved != input {
            changes.push(SessionChange { key: (*key).to_string(), before: input, after: resolved });
            continue;
        }
        if !resolved.is_empty() && !has_han(&input) {
            verified.push((*key).to_string());
            continue;
        }
        if has_han(&input) {
            notes.push(SessionNote { key: (*key).to_string(), note: "未找到可靠的英文对应, 请核对.".into() });
        } else if resolved.is_empty() && matches!(*key, "performer" | "conductor" | "composer" | "orchestra" | "venue" | "city") {
            notes.push(SessionNote { key: (*key).to_string(), note: "目录暂无此名称, 请核对拼写.".into() });
        }
    }
    SessionNormalize { changes, notes, verified }
}

fn session_title(value: &str, context: &dyn Fn(&str) -> String) -> String {
    if let Some(direct) = dict_get(EVENTS, value) {
        return direct.to_string();
    }
    let mut title = value.to_string();
    let mut subject = String::new();
    for key in ["performer", "conductor", "orchestra"] {
        let raw = context(key);
        let map = if key == "orchestra" { ORCHESTRAS } else { PEOPLE };
        let resolved = dict_get(map, &raw).map(str::to_string).or_else(|| if !raw.is_empty() && !has_han(&raw) { Some(raw.clone()) } else { None });
        if let Some(resolved) = resolved {
            if title.starts_with(&raw) && title != raw {
                subject = resolved;
                title = title[raw.len()..].trim_start_matches(|ch: char| ch.is_whitespace() || matches!(ch, '·' | ':' | '：' | '–' | '—' | '-')).to_string();
                break;
            }
        }
    }
    let Some((suffix_name, suffix_en)) = TITLE_TERMS.iter().find(|(name, _)| title.ends_with(name)) else {
        return String::new();
    };
    let pre = title[..title.len() - suffix_name.len()].trim().to_string();
    if pre.is_empty() && !subject.is_empty() {
        return format!("{subject} – {suffix_en}");
    }
    if pre.is_empty() {
        return (*suffix_en).to_string();
    }
    if let Some(named) = dict_get(PEOPLE, &pre).or_else(|| dict_get(ORCHESTRAS, &pre)) {
        return [Some(subject.as_str()), Some(named), Some(*suffix_en)].into_iter().flatten().filter(|item| !item.is_empty()).collect::<Vec<_>>().join(" – ");
    }
    if let Some(composer) = composer_name(pre.trim_end_matches('的').trim()) {
        let last = composer.split(' ').next_back().unwrap_or(composer);
        return if subject.is_empty() { format!("{last} {suffix_en}") } else { format!("{subject} – {last} {suffix_en}") };
    }
    if !has_han(&pre) && pre.chars().count() <= 170 && pre.chars().all(|ch| ch.is_alphanumeric() || matches!(ch, ' ' | '.' | '·' | '\'' | '’' | '&' | '–' | '-')) {
        return [Some(subject.as_str()), Some(pre.as_str()), Some(*suffix_en)].into_iter().flatten().filter(|item| !item.is_empty()).collect::<Vec<_>>().join(" – ");
    }
    String::new()
}

fn normalize_list(value: &str, find: impl Fn(&str) -> Option<&str>) -> String {
    if let Some(exact) = find(value) {
        return exact.to_string();
    }
    let parts: Vec<&str> = value.split(|ch: char| matches!(ch, '、' | '；' | ';' | '/' | '，' | ',')).map(str::trim).filter(|item| !item.is_empty()).collect();
    if parts.len() < 2 {
        return String::new();
    }
    let mut resolved = Vec::new();
    for part in parts {
        let Some(name) = find(part) else {
            return String::new();
        };
        if !resolved.contains(&name) {
            resolved.push(name);
        }
    }
    resolved.join(" / ")
}

fn composer_name(value: &str) -> Option<&str> {
    dict_get(COMPOSERS, value)
}

struct Hall {
    city: &'static str,
    venues: &'static [(&'static str, &'static [&'static str])],
}

fn location_for(value: &str) -> Option<&'static Hall> {
    HALLS.iter().find(|hall| dict_get(hall.venues, value).is_some())
}

fn dict_get<'a>(entries: &'a [(&'a str, &'a [&'a str])], value: &str) -> Option<&'a str> {
    let key = lookup_key(value);
    if key.is_empty() {
        return None;
    }
    entries.iter().find(|(_, names)| names.iter().any(|name| lookup_key(name) == key)).map(|(english, _)| *english)
}

fn clean(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn lookup_key(value: &str) -> String {
    clean(value)
        .chars()
        .filter(|ch| !ch.is_whitespace() && !is_mark(*ch) && !is_punct(*ch))
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn is_mark(ch: char) -> bool {
    matches!(ch, '\u{0300}'..='\u{036f}' | '\u{00b4}' | '\u{00a8}')
}

fn is_punct(ch: char) -> bool {
    ch.is_ascii_punctuation() || matches!(ch, '·' | '、' | '，' | '。' | '；' | '：' | '！' | '？' | '（' | '）' | '—' | '–' | '“' | '”' | '‘' | '’')
}

fn has_han(value: &str) -> bool {
    value.chars().any(|ch| matches!(ch, '\u{3400}'..='\u{9fff}'))
}

fn valid_date(value: &str) -> bool {
    let parts: Vec<_> = value.split('-').collect();
    if parts.len() != 3 || parts[0].len() != 4 || parts[1].len() != 2 || parts[2].len() != 2 {
        return false;
    }
    let Ok(year) = parts[0].parse::<i32>() else { return false };
    let Ok(month) = parts[1].parse::<u32>() else { return false };
    let Ok(day) = parts[2].parse::<u32>() else { return false };
    let leap = year % 400 == 0 || (year % 4 == 0 && year % 100 != 0);
    let max = [0, 31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12).contains(&month) && day >= 1 && day <= max[month as usize]
}

const PEOPLE: &[(&str, &[&str])] = &[
    ("Michel Dalberto", &["Michel Dalberto", "米歇尔·达尔贝托", "米歇尔·达尔贝多", "米歇尔·达尔伯托", "达尔贝托", "达尔贝多"]),
    ("Dénes Várjon", &["Dénes Várjon", "Denes Varjon", "德内什·瓦尔扬", "德内什·瓦尔约恩", "德奈什·瓦尔扬", "德内什·瓦尔荣", "瓦尔扬", "瓦尔荣"]),
    ("Dmitri Alexeev", &["Dmitri Alexeev", "德米特里·阿列克谢耶夫", "德米特里·阿列克谢夫"]),
    ("Hans Zender", &["Hans Zender", "汉斯·岑德尔", "汉斯·森德尔", "汉斯·曾德尔", "汉斯·泽德"]),
    ("Franz Welser-Möst", &["Franz Welser-Möst", "Franz Welser-Most", "弗朗茨·韦尔泽-莫斯特", "弗朗茨·威尔瑟-莫斯特", "韦尔泽-莫斯特"]),
    ("Michael Sanderling", &["Michael Sanderling", "米夏埃尔·桑德林", "迈克尔·桑德林"]),
    ("Ingo Metzmacher", &["Ingo Metzmacher", "英戈·梅茨马赫"]),
    ("Seiji Ozawa", &["Seiji Ozawa", "小泽征尔", "小澤征爾"]),
    ("Yehudi Menuhin", &["Yehudi Menuhin", "耶胡迪·梅纽因", "梅纽因"]),
    ("Ernest Ansermet", &["Ernest Ansermet", "欧内斯特·安塞美", "安塞美"]),
    ("Mikhail Voskresensky", &["Mikhail Voskresensky", "米哈伊尔·沃斯克列先斯基", "沃斯克列先斯基"]),
];

const CITIES: &[(&str, &[&str])] = &[
    ("Shanghai", &["Shanghai", "上海", "上海市"]),
    ("Harbin", &["Harbin", "哈尔滨", "哈爾濱", "哈尔滨市"]),
    ("Beijing", &["Beijing", "北京", "北京市"]),
    ("Hamburg", &["Hamburg", "汉堡", "漢堡"]),
    ("Vienna", &["Vienna", "维也纳", "維也納"]),
    ("Montreux", &["Montreux", "蒙特勒", "蒙特鲁"]),
    ("Lucerne", &["Lucerne", "琉森", "卢塞恩", "盧塞恩"]),
    ("Zurich", &["Zurich", "Zürich", "苏黎世", "蘇黎世"]),
    ("Munich", &["Munich", "慕尼黑"]),
    ("Lausanne", &["Lausanne", "洛桑"]),
    ("Paris", &["Paris", "巴黎"]),
    ("London", &["London", "伦敦", "倫敦"]),
    ("Berlin", &["Berlin", "柏林"]),
    ("Tokyo", &["Tokyo", "东京", "東京"]),
    ("New York", &["New York", "纽约", "紐約", "纽约市"]),
    ("Boston", &["Boston", "波士顿", "波士頓"]),
    ("Guangzhou", &["Guangzhou", "广州", "廣州", "广州市"]),
    ("Shenzhen", &["Shenzhen", "深圳", "深圳市"]),
    ("Hong Kong", &["Hong Kong", "香港"]),
];

const HALLS: &[Hall] = &[
    Hall { city: "Shanghai", venues: &[("Shanghai Symphony Hall", &["Shanghai Symphony Hall", "上海交响音乐厅", "上海交响乐团音乐厅", "上交音乐厅"]), ("Shanghai Concert Hall", &["Shanghai Concert Hall", "上海音乐厅"]), ("Cadillac Shanghai Concert Hall", &["Cadillac Shanghai Concert Hall", "凯迪拉克上海音乐厅", "Cadillac · Shanghai Concert Hall"]), ("Shanghai Grand Theatre", &["Shanghai Grand Theatre", "上海大剧院"])] },
    Hall { city: "Beijing", venues: &[("National Centre for the Performing Arts", &["National Centre for the Performing Arts", "国家大剧院", "中國國家大劇院", "中国国家大剧院", "NCPA"])] },
    Hall { city: "Vienna", venues: &[("Musikverein", &["Musikverein", "维也纳音乐协会", "Vienna Musikverein"]), ("Golden Hall, Musikverein", &["Golden Hall, Musikverein", "维也纳金色大厅", "维也纳音乐协会金色大厅"])] },
    Hall { city: "Lucerne", venues: &[("KKL Luzern", &["KKL Luzern", "琉森文化会议中心", "卢塞恩文化会议中心", "KKL Lucerne"])] },
    Hall { city: "Hamburg", venues: &[("Elbphilharmonie Hamburg", &["Elbphilharmonie Hamburg", "易北爱乐音乐厅", "Elbphilharmonie"])] },
    Hall { city: "New York", venues: &[("Carnegie Hall", &["Carnegie Hall", "卡内基音乐厅"])] },
];

const ORCHESTRAS: &[(&str, &[&str])] = &[
    ("Shanghai Symphony Orchestra", &["Shanghai Symphony Orchestra", "上海交响乐团", "上海交響樂團", "上交"]),
    ("Hamburger Symphoniker", &["Hamburger Symphoniker", "汉堡交响乐团", "德国汉堡交响乐团", "Hamburg Symphony Orchestra"]),
    ("Cleveland Orchestra", &["Cleveland Orchestra", "克利夫兰管弦乐团", "克利夫兰交响乐团", "The Cleveland Orchestra"]),
    ("Vienna Philharmonic", &["Vienna Philharmonic", "维也纳爱乐乐团", "Wiener Philharmoniker"]),
    ("Berliner Philharmoniker", &["Berliner Philharmoniker", "柏林爱乐乐团", "柏林愛樂樂團", "Berlin Philharmonic"]),
    ("Lucerne Symphony Orchestra", &["Lucerne Symphony Orchestra", "琉森交响乐团", "卢塞恩交响乐团", "Luzerner Sinfonieorchester"]),
    ("Boston Symphony Orchestra", &["Boston Symphony Orchestra", "波士顿交响乐团", "波士頓交響樂團", "BSO"]),
];

const ORIGINS: &[(&str, &[&str])] = &[
    ("Personal Live Recording", &["Personal Live Recording", "现场自录", "现场录音（自录）", "本人现场录制", "观众自录"]),
    ("Live Recording", &["Live Recording", "现场录音", "现场", "現場錄音"]),
    ("Historical Live Recording", &["Historical Live Recording", "历史现场录音"]),
    ("Radio Broadcast", &["Radio Broadcast", "广播", "广播录音", "电台广播", "电台录音", "广播实况", "電台錄音"]),
    ("Television Broadcast", &["Television Broadcast", "电视广播", "电视录音"]),
    ("Commercial Release", &["Commercial Release", "商业发行", "唱片发行"]),
    ("Private Recording", &["Private Recording", "私人录音"]),
    ("Archival Recording", &["Archival Recording", "档案录音", "档案馆录音"]),
    ("Studio Recording", &["Studio Recording", "录音室录音"]),
    ("Online Stream", &["Online Stream", "网络直播", "网络回放", "网络录音"]),
];

const EVENTS: &[(&str, &[&str])] = &[
    ("Piano Recital", &["Piano Recital", "钢琴独奏音乐会", "钢琴独奏会", "钢琴独奏专场", "钢琴音乐会"]),
    ("Solo Recital", &["Solo Recital", "独奏音乐会", "独奏会"]),
    ("Violin Recital", &["Violin Recital", "小提琴独奏会", "小提琴独奏音乐会"]),
    ("Piano Masterclass", &["Piano Masterclass", "钢琴大师课", "钢琴大师班"]),
    ("Masterclass", &["Masterclass", "大师课", "大师班"]),
    ("Symphony Concert", &["Symphony Concert", "交响音乐会", "交响乐音乐会"]),
    ("Chamber Music Concert", &["Chamber Music Concert", "室内乐音乐会"]),
    ("Concert", &["Concert", "音乐会"]),
    ("Live Recording", &["Live Recording", "现场录音"]),
];

const TITLE_TERMS: &[(&str, &str)] = &[
    ("钢琴独奏音乐会", "Piano Recital"),
    ("钢琴独奏会", "Piano Recital"),
    ("钢琴独奏专场", "Piano Recital"),
    ("钢琴大师课", "Piano Masterclass"),
    ("小提琴独奏会", "Violin Recital"),
    ("独奏音乐会", "Solo Recital"),
    ("独奏会", "Solo Recital"),
    ("大师课", "Masterclass"),
    ("交响音乐会", "Symphony Concert"),
    ("室内乐音乐会", "Chamber Music Concert"),
    ("音乐会", "Concert"),
];

const COMPOSERS: &[(&str, &[&str])] = &[
    ("Ludwig van Beethoven", &["Ludwig van Beethoven", "Beethoven", "贝多芬"]),
    ("Wolfgang Amadeus Mozart", &["Wolfgang Amadeus Mozart", "Mozart", "莫扎特"]),
    ("Frédéric Chopin", &["Frédéric Chopin", "Frederic Chopin", "Chopin", "肖邦"]),
    ("Robert Schumann", &["Robert Schumann", "Schumann", "舒曼"]),
    ("Franz Schubert", &["Franz Schubert", "Schubert", "舒伯特"]),
    ("Claude Debussy", &["Claude Debussy", "Debussy", "德彪西"]),
    ("Anton Bruckner", &["Anton Bruckner", "Bruckner", "布鲁克纳"]),
    ("Johann Sebastian Bach", &["Johann Sebastian Bach", "Bach", "巴赫"]),
    ("Pyotr Ilyich Tchaikovsky", &["Pyotr Ilyich Tchaikovsky", "Tchaikovsky", "柴可夫斯基", "柴科夫斯基"]),
    ("Sergei Rachmaninoff", &["Sergei Rachmaninoff", "Rachmaninoff", "拉赫玛尼诺夫", "拉赫玛尼洛夫"]),
    ("Gustav Mahler", &["Gustav Mahler", "Mahler", "马勒"]),
    ("Johannes Brahms", &["Johannes Brahms", "Brahms", "勃拉姆斯"]),
    ("Maurice Ravel", &["Maurice Ravel", "Ravel", "拉威尔"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translates_known_session_fields_and_leaves_unknown_names() {
        let review = normalize(&[
            ("name", "钢琴独奏音乐会"),
            ("city", "上海"),
            ("venue", "上海交响音乐厅"),
            ("orchestra", "上海交响乐团"),
            ("performer", "米歇尔·达尔贝托"),
            ("origin", "现场自录"),
            ("composer", "贝多芬"),
            ("date", "2024-05-02"),
            ("conductor", "某位指挥"),
        ]);
        assert_eq!(review.changes.iter().find(|item| item.key == "name").unwrap().after, "Piano Recital");
        assert_eq!(review.changes.iter().find(|item| item.key == "city").unwrap().after, "Shanghai");
        assert_eq!(review.changes.iter().find(|item| item.key == "venue").unwrap().after, "Shanghai Symphony Hall");
        assert_eq!(review.changes.iter().find(|item| item.key == "orchestra").unwrap().after, "Shanghai Symphony Orchestra");
        assert_eq!(review.changes.iter().find(|item| item.key == "performer").unwrap().after, "Michel Dalberto");
        assert_eq!(review.changes.iter().find(|item| item.key == "origin").unwrap().after, "Personal Live Recording");
        assert_eq!(review.changes.iter().find(|item| item.key == "composer").unwrap().after, "Ludwig van Beethoven");
        assert!(review.verified.iter().any(|item| item == "date"));
        assert!(review.notes.iter().any(|item| item.key == "conductor"));
    }

    #[test]
    fn flags_a_hall_that_belongs_to_another_city() {
        let review = normalize(&[("city", "北京"), ("venue", "上海音乐厅")]);
        assert!(review.notes.iter().any(|item| item.note.contains("Shanghai")));
        assert!(review.changes.is_empty());
    }
}
