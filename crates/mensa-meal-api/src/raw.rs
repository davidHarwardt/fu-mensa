
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Clone)]
pub struct ApiQuery {
    mensa_id: String,
    json: String,
    mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    lang: Option<String>,
}

impl ApiQuery {
    pub fn new(
        mensa: impl Into<String>,
        lang: Option<impl Into<String>>
    ) -> Self {
        Self {
            mensa_id: mensa.into(),
            lang: lang.map(Into::into),
            json: "1".into(),
            mode: "slsys".into(),
        }
    }
}


#[derive(Debug, Deserialize, Clone)]
pub struct ApiResult {
    pub result: Vec<ApiPlan>,
    pub mensaname: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiPlan {
    pub tag: ApiDay,
    pub essen: Vec<ApiMeal>,
}

#[derive(Debug, Deserialize, Clone)]
/// should prob use timestamp and datum_iso
pub struct ApiDay {
    /// unix timestamp
    pub timestamp: u64,
    pub datum_iso: String,

    pub tag_formatiert: String,
    pub tag_formatiert2: String,
    pub tag_formatiert_rel: String,
    pub jahrestag: String,
    pub wochentag: String,
    pub wochentag_short: String,
    pub datum: String,
    pub datum2: String,
    pub wota_index: String,
    pub kw: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiMeal {
    pub category: String,
    pub title: String,
    pub description: String,
    /// alergens, ...
    pub kennzeichnungen: String,
    pub preis1: String,
    pub preis2: String,
    pub preis3: String,

    pub ampel: String,
    pub co2_wert: String,
    pub co2_bewertung: String,
    pub h2o_wert: String,
    pub h2o_bewertung: String,
    pub attributes: ApiMealAttributes,
    pub title_orig: String,
    #[serde(rename = "alreadyExtracted_title")]
    pub already_extracted_title: bool,
    pub title_clean: String,
    // skip: icons, icons_kuerzel, icons2
    pub title2: String,
    #[serde(rename = "alreadyExtracted_description")]
    pub already_extracted_description: bool,
    pub description_clean: String,
    #[serde(rename = "md5Source")]
    pub md5_source: String,
    pub md5: String,
    pub kat_id: String,
    pub loc_id: String,
    pub pfand: f64,
    // skip all preis
    pub a_id: String,
    #[serde(rename = "kennzRest")] pub kennz_rest: String,
    pub preis_vorhanden: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ApiMealAttributes {
    #[serde(rename = "produktionId")] pub produktion_id: String,
    #[serde(rename = "artikelId")] pub artikel_id: String,
    #[serde(rename = "dispoId")] pub dispo_id: String,
}




