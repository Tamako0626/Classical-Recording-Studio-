//! 一小份离线曲目别名, 用来把常见中文简称收成英文曲名.

#[derive(Clone, Copy, Debug)]
pub struct Suggestion {
    pub composer: &'static str,
    pub title: &'static str,
}

struct Work {
    composer: &'static str,
    title: &'static str,
    aliases: &'static [&'static str],
}

const WORKS: &[Work] = &[
    Work { composer: "Ludwig van Beethoven", title: "Symphony No. 3 in E-flat major, Op. 55, Eroica", aliases: &["贝三", "英雄交响曲", "贝多芬第三交响曲"] },
    Work { composer: "Ludwig van Beethoven", title: "Symphony No. 5 in C minor, Op. 67", aliases: &["贝五", "命运"] },
    Work { composer: "Ludwig van Beethoven", title: "Symphony No. 6 in F major, Op. 68, Pastoral", aliases: &["贝六", "田园"] },
    Work { composer: "Ludwig van Beethoven", title: "Symphony No. 7 in A major, Op. 92", aliases: &["贝七"] },
    Work { composer: "Ludwig van Beethoven", title: "Symphony No. 9 in D minor, Op. 125, Choral", aliases: &["贝九", "合唱交响曲"] },
    Work { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 14 in C-sharp minor, Op. 27 No. 2, Moonlight", aliases: &["月光奏鸣曲", "贝多芬月光"] },
    Work { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 30 in E major, Op. 109", aliases: &["贝多芬第三十钢琴奏鸣曲"] },
    Work { composer: "Wolfgang Amadeus Mozart", title: "Piano Sonata No. 11 in A major, K. 331", aliases: &["土耳其进行曲奏鸣曲"] },
    Work { composer: "Wolfgang Amadeus Mozart", title: "Fantasia in C minor, K. 475", aliases: &["莫扎特幻想曲"] },
    Work { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 6 in F major, Op. 10 No. 2", aliases: &["贝多芬第六奏鸣曲", "贝多芬第六钢琴奏鸣曲"] },
    Work { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 7 in D major, Op. 10 No. 3", aliases: &["贝多芬第七奏鸣曲", "贝多芬第七钢琴奏鸣曲"] },
    Work { composer: "Wolfgang Amadeus Mozart", title: "Piano Sonata No. 8 in A minor, K. 310", aliases: &["莫扎特第八钢琴奏鸣曲"] },
    Work { composer: "Wolfgang Amadeus Mozart", title: "Piano Sonata No. 14 in C minor, K. 457", aliases: &["莫扎特第十四钢琴奏鸣曲"] },
    Work { composer: "Wolfgang Amadeus Mozart", title: "Piano Concerto No. 24 in C minor, K. 491", aliases: &["莫扎特第二十四钢琴协奏曲"] },
    Work { composer: "Frédéric Chopin", title: "Piano Concerto No. 1 in E minor, Op. 11", aliases: &["肖协1", "肖一钢协"] },
    Work { composer: "Frédéric Chopin", title: "Piano Concerto No. 2 in F minor, Op. 21", aliases: &["肖协2", "肖二钢协"] },
    Work { composer: "Frédéric Chopin", title: "Mazurkas, Op. 7", aliases: &["肖邦Op7", "肖邦玛祖卡Op.7"] },
    Work { composer: "Frédéric Chopin", title: "Nocturne in B major, Op. 62 No. 1", aliases: &["肖邦夜曲Op62-1"] },
    Work { composer: "Frédéric Chopin", title: "Barcarolle in F-sharp major, Op. 60", aliases: &["肖邦船歌", "船歌"] },
    Work { composer: "Robert Schumann", title: "Piano Concerto in A minor, Op. 54", aliases: &["舒曼钢协"] },
    Work { composer: "Franz Schubert", title: "Symphony No. 9 in C major, D. 944, The Great", aliases: &["舒九", "伟大交响曲"] },
    Work { composer: "Claude Debussy", title: "L'isle joyeuse, L. 106", aliases: &["欢乐岛"] },
    Work { composer: "Anton Bruckner", title: "Symphony No. 7 in E major, WAB 107", aliases: &["布七"] },
    Work { composer: "Johann Sebastian Bach", title: "English Suite No. 2 in A minor, BWV 807", aliases: &["巴赫第二英国组曲"] },
    Work { composer: "Robert Schumann", title: "Kinderszenen, Op. 15", aliases: &["童年情景"] },
    Work { composer: "Robert Schumann", title: "Fantasie in C major, Op. 17", aliases: &["舒曼C大调幻想曲"] },
    Work { composer: "Robert Schumann", title: "Symphonic Etudes, Op. 13", aliases: &["交响练习曲"] },
    Work { composer: "Franz Schubert", title: "Symphony No. 8 in B minor, D. 759, Unfinished", aliases: &["舒伯特未完成", "未完成交响曲"] },
    Work { composer: "Claude Debussy", title: "Suite bergamasque: III. Clair de lune", aliases: &["德彪西月光", "Clair de lune"] },
    Work { composer: "Pyotr Ilyich Tchaikovsky", title: "Symphony No. 6 in B minor, Op. 74, Pathétique", aliases: &["柴六", "悲怆交响曲"] },
    Work { composer: "Sergei Rachmaninoff", title: "Piano Concerto No. 2 in C minor, Op. 18", aliases: &["拉二钢协", "拉赫二协"] },
    Work { composer: "Gustav Mahler", title: "Symphony No. 2 in C minor, Resurrection", aliases: &["马二", "复活交响曲"] },
    Work { composer: "Johannes Brahms", title: "Piano Concerto No. 2 in B-flat major, Op. 83", aliases: &["勃拉姆斯二协"] },
    Work { composer: "Maurice Ravel", title: "Gaspard de la nuit", aliases: &["夜之加斯帕德", "加斯帕德"] },
    Work { composer: "Robert Schumann", title: "Fantasiestücke, Op. 12", aliases: &["幻想曲集", "舒曼幻想曲集"] },
];

pub fn suggest_work(text: &str) -> Option<Suggestion> {
    search_works(text).into_iter().next()
}

pub fn search_works(text: &str) -> Vec<Suggestion> {
    let folded = text.trim().to_lowercase();
    if folded.chars().count() < 2 {
        return Vec::new();
    }
    WORKS
        .iter()
        .filter(|work| {
            let title = work.title.to_lowercase();
            work.aliases.iter().any(|alias| folded.contains(&alias.to_lowercase())) || folded.contains(&title) || folded.chars().count() >= 6 && title.contains(&folded)
        })
        .map(|work| Suggestion { composer: work.composer, title: work.title })
        .collect()
}

#[derive(Clone, Copy, Debug)]
pub struct MovementList {
    pub composer: &'static str,
    pub title: &'static str,
    pub parts: &'static [&'static str],
    pub main: usize,
    pub numbered: bool,
}

const MOVEMENTS: &[MovementList] = &[
    MovementList { composer: "Ludwig van Beethoven", title: "Symphony No. 3 in E-flat major, Op. 55, Eroica", parts: &["Allegro con brio", "Marcia funebre: Adagio assai", "Scherzo: Allegro vivace", "Finale: Allegro molto"], main: 4, numbered: false },
    MovementList { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 6 in F major, Op. 10 No. 2", parts: &["Allegro", "Allegretto", "Presto"], main: 3, numbered: false },
    MovementList { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 7 in D major, Op. 10 No. 3", parts: &["Presto", "Largo e mesto", "Menuetto: Allegro", "Rondo: Allegro"], main: 4, numbered: false },
    MovementList { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 14 in C-sharp minor, Op. 27 No. 2, Moonlight", parts: &["Adagio sostenuto", "Allegretto", "Presto agitato"], main: 3, numbered: false },
    MovementList { composer: "Ludwig van Beethoven", title: "Piano Sonata No. 30 in E major, Op. 109", parts: &["Vivace, ma non troppo – Adagio espressivo", "Prestissimo", "Gesangvoll, mit innigster Empfindung"], main: 3, numbered: false },
    MovementList { composer: "Robert Schumann", title: "Fantasiestücke, Op. 12", parts: &["Des Abends", "Aufschwung", "Warum?", "Grillen", "In der Nacht", "Fabel", "Traumes Wirren", "Ende vom Lied"], main: 8, numbered: false },
    MovementList { composer: "Robert Schumann", title: "Symphonic Etudes, Op. 13", parts: &["Theme: Andante", "Etude I (Variation 1)", "Etude II (Variation 2)", "Etude III: Vivace", "Etude IV (Variation 3)", "Etude V (Variation 4)", "Etude VI (Variation 5)", "Etude VII (Variation 6)", "Etude VIII (Variation 7)", "Etude IX: Presto possibile", "Etude X (Variation 8)", "Etude XI (Variation 9)", "Etude XII (Finale): Allegro brillante", "Posthumous Variation I", "Posthumous Variation II", "Posthumous Variation III", "Posthumous Variation IV", "Posthumous Variation V"], main: 13, numbered: true },
    MovementList { composer: "Robert Schumann", title: "Kinderszenen, Op. 15", parts: &["Von fremden Ländern und Menschen", "Kuriose Geschichte", "Hasche-Mann", "Bittendes Kind", "Glückes genug", "Wichtige Begebenheit", "Träumerei", "Am Kamin", "Ritter vom Steckenpferd", "Fast zu ernst", "Fürchtenmachen", "Kind im Einschlummern", "Der Dichter spricht"], main: 13, numbered: false },
    MovementList { composer: "Maurice Ravel", title: "Gaspard de la nuit", parts: &["Ondine", "Le Gibet", "Scarbo"], main: 3, numbered: false },
    MovementList { composer: "Wolfgang Amadeus Mozart", title: "Piano Sonata No. 11 in A major, K. 331", parts: &["Andante grazioso (Theme and Variations)", "Menuetto and Trio", "Alla turca: Allegretto"], main: 3, numbered: false },
    MovementList { composer: "Wolfgang Amadeus Mozart", title: "Piano Sonata No. 8 in A minor, K. 310", parts: &["Allegro maestoso", "Andante cantabile con espressione", "Presto"], main: 3, numbered: false },
    MovementList { composer: "Wolfgang Amadeus Mozart", title: "Piano Sonata No. 14 in C minor, K. 457", parts: &["Molto allegro", "Adagio", "Allegro assai"], main: 3, numbered: false },
    MovementList { composer: "Johann Sebastian Bach", title: "English Suite No. 2 in A minor, BWV 807", parts: &["Prelude", "Allemande", "Courante", "Sarabande", "Bourrée I and II", "Gigue"], main: 6, numbered: false },
    MovementList { composer: "Frédéric Chopin", title: "Piano Concerto No. 2 in F minor, Op. 21", parts: &["Maestoso", "Larghetto", "Allegro vivace"], main: 3, numbered: false },
    MovementList { composer: "Pyotr Ilyich Tchaikovsky", title: "Symphony No. 6 in B minor, Op. 74, Pathétique", parts: &["Adagio – Allegro non troppo", "Allegro con grazia", "Allegro molto vivace", "Adagio lamentoso"], main: 4, numbered: false },
];

const ROMAN: &[&str] = &["I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X", "XI", "XII", "XIII", "XIV", "XV", "XVI", "XVII", "XVIII"];

pub fn find_movements(title: &str, composer: &str) -> Option<&'static MovementList> {
    let query = title.trim();
    if query.chars().count() < 2 {
        return None;
    }
    if let Some(hit) = suggest_work(query) {
        if let Some(list) = MOVEMENTS.iter().find(|item| item.title == hit.title) {
            if composer_fits(composer, list.composer) {
                return Some(list);
            }
        }
    }
    let folded = query.to_lowercase();
    MOVEMENTS.iter().find(|item| composer_fits(composer, item.composer) && (folded.contains(&item.title.to_lowercase()) || item.title.to_lowercase().contains(&folded) && query.chars().count() >= 8))
}

pub fn movement_title(list: &MovementList, index: usize) -> String {
    let name = list.parts.get(index).copied().unwrap_or("");
    if list.numbered {
        format!("{} – {name}", list.title)
    } else {
        let mark = ROMAN.get(index).copied().unwrap_or("?");
        format!("{} – {mark}. {name}", list.title)
    }
}

fn composer_fits(hint: &str, known: &str) -> bool {
    let hint = hint.trim();
    if hint.is_empty() {
        return true;
    }
    let lower = hint.to_lowercase();
    let known = known.to_lowercase();
    if lower.split_whitespace().any(|token| token.chars().count() >= 3 && known.contains(token)) || known.split_whitespace().any(|token| token.chars().count() >= 4 && lower.contains(token)) {
        return true;
    }
    const NAMES: &[(&str, &str)] = &[("贝多芬", "beethoven"), ("莫扎特", "mozart"), ("肖邦", "chopin"), ("舒曼", "schumann"), ("舒伯特", "schubert"), ("德彪西", "debussy"), ("巴赫", "bach"), ("拉威尔", "ravel"), ("柴可夫斯基", "tchaikovsky")];
    if let Some((_, english)) = NAMES.iter().find(|(name, _)| hint.contains(name)) {
        return known.contains(english);
    }
    hint.chars().any(|ch| ('\u{3400}'..='\u{9fff}').contains(&ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_a_common_alias() {
        let hit = suggest_work("今晚的月光奏鸣曲").unwrap();
        assert!(hit.title.contains("Moonlight"));
        assert_eq!(hit.composer, "Ludwig van Beethoven");
        assert!(suggest_work("肖协2").unwrap().title.contains("Op. 21"));
        assert!(suggest_work("布七").unwrap().title.contains("WAB 107"));
    }

    #[test]
    fn movement_lookup_lists_the_moonlight_sonata() {
        let list = find_movements("月光奏鸣曲", "贝多芬").unwrap();
        assert_eq!(list.parts.len(), 3);
        assert!(movement_title(list, 0).contains("Adagio sostenuto"));
        assert!(find_movements("月光奏鸣曲", "Mozart").is_none());
    }
}
