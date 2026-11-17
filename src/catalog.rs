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
];

pub fn suggest_work(text: &str) -> Option<Suggestion> {
    let folded = text.trim().to_lowercase();
    if folded.is_empty() {
        return None;
    }
    WORKS.iter().find(|work| work.aliases.iter().any(|alias| folded.contains(&alias.to_lowercase())) || folded.contains(&work.title.to_lowercase())).map(|work| Suggestion {
        composer: work.composer,
        title: work.title,
    })
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
}
