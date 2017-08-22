use super::fns::*;

#[derive(Deserialize, Clone, Debug)]
pub struct CorpusData {
    #[serde(rename = "TIPLOCDATA")]
    pub tiploc_data: Vec<CorpusEntry>
}
#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub struct CorpusEntry {
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub stanox: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub uic: Option<String>,
    #[serde(rename = "3ALPHA", deserialize_with = "non_empty_str_opt")]
    pub crs: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub tiploc: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub nlc: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub nlcdesc: Option<String>,
    #[serde(deserialize_with = "non_empty_str_opt")]
    pub nlcdesc16: Option<String>
}
impl CorpusEntry {
    pub fn contains_data(&self) -> bool {
        self.stanox.is_some() ||
            self.uic.is_some() ||
            self.crs.is_some() ||
            self.tiploc.is_some() ||
            self.nlc.is_some()
    }
}
