use std::{borrow::Cow, collections::{HashMap, HashSet}, future::Future};

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use super::raw;

#[derive(Debug, Default, Serialize, Deserialize, Clone)]
pub struct MealPlans {
    mensas: HashMap<String, MealPlan>,
}

impl MealPlans {
    pub fn get(&self, mensa: &str, lang: Option<&str>) -> Option<&MealPlan> {
        self.mensas.get(&Self::key(mensa, lang))
    }

    pub fn insert(
        &mut self, mensa: &str,
        lang: Option<&str>, plan: MealPlan
    ) -> Option<MealPlan> {
        self.mensas.insert(Self::key(mensa, lang), plan)
    }

    pub fn key(mensa: &str, lang: Option<&str>) -> String {
        const DEFAULT_LANG: &str = "en";
        format!("{};{mensa}", lang.unwrap_or(DEFAULT_LANG))
    }

    pub fn mensas(&self) -> impl Iterator<Item = (&str, &str)> {
        self.mensas.keys().flat_map(|v| v.split_once(";"))
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MealPlan {
    /// is always sorted
    days: Vec<MealDay>,
    mensa_name: String,
}

impl MealPlan {
    pub fn new(mensa_name: String) -> Self {
        Self { mensa_name, days: Vec::new() }
    }

    #[inline]
    fn assert_sorted(&self) {
        debug_assert!(self.days
            .windows(2)
            .all(|v| v[0].date <= v[1].date)
        );
    }

    pub fn add_day(
        &mut self, day: NaiveDate, meals: MealDay,
    ) {
        self.assert_sorted();
        match self.days.binary_search_by_key(&day, |v| v.date) {
            // is already contained
            Ok(_) =>
                tracing::info!("skipping {day}, already in list"),
            // insert at i
            Err(i) => self.days.insert(i, meals),
        }
    }

    pub fn remove_day(&mut self, day: NaiveDate) {
        self.assert_sorted();
        match self.days.binary_search_by_key(&day, |v| v.date) {
            Ok(v) => { self.days.remove(v); },
            Err(_) =>
                tracing::warn!("tried to remove nonexistant day: {day}"),
        }
    }

    pub fn mensa(&self) -> &str { &self.mensa_name }

    pub fn get_day_internal(&self, day: &NaiveDate) -> Option<&MealDay> {
        match self.days.binary_search_by_key(day, |v| v.date) {
            Ok(i) => self.days.get(i),
            Err(_) => None,
        }
    }

    pub async fn get_day<F: Future<Output = Option<MealDay>>>(
        &self, day: &NaiveDate,
        req: impl FnOnce(&NaiveDate) -> F,
    ) -> Option<Cow<'_, MealDay>> {
        if let Some(v) = self.get_day_internal(day) {
            Some(Cow::Borrowed(v))
        } else { req(day).await.map(Cow::Owned) }
    }

    pub fn days(&self) -> std::slice::Iter<'_, MealDay> {
        self.days.iter()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MealPlanParseError {
    #[error("InvalidDate: {0}")]
    InvalidDate(#[from] chrono::ParseError),
}

impl TryFrom<raw::ApiResult> for MealPlan {
    type Error = MealPlanParseError;

    fn try_from(value: raw::ApiResult) -> Result<Self, Self::Error> {

        let mut days = value.result.into_iter().map(|v| {
            let mut categories = HashMap::<String, Vec<_>>::new();
            for meal in v.essen {
                let desc = meal.description_clean.trim().to_string();

                categories.entry(meal.category).or_default().push(MensaMeal {
                    title: meal.title_clean,
                    description: if desc.is_empty() { None } else { Some(desc) },
                    price: if meal.preis_vorhanden { (|| {
                        Some(MealPrice {
                            students: Price::parse(&meal.preis1)?,
                            servants: Price::parse(&meal.preis2)?,
                            guests: Price::parse(&meal.preis3)?,
                        })
                    })() } else { None },
                    info: MealInfo::parse(&meal.kennzeichnungen),
                    id: meal.attributes.artikel_id,
                });
            }

            Ok(MealDay {
                date: NaiveDate::parse_from_str(&v.tag.datum_iso, "%Y-%m-%d")?,
                categories,
            })
        }).collect::<Result<Vec<MealDay>, MealPlanParseError>>()?;

        days.sort_by_key(|v| v.date);

        Ok(Self {
            days,
            mensa_name: value.mensaname,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MealDay {
    pub date: chrono::NaiveDate,
    pub categories: HashMap<String, Vec<MensaMeal>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MensaMeal {
    // should prob. be title_orig or title_clean
    title: String,
    description: Option<String>,
    // exist if preis_vorhanden is true?
    price: Option<MealPrice>,
    info: MealInfo,
    // use article id or sth. to uniqely identify
    id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MealInfo {
    env_rating: MealEnvRating,
    addatives: HashSet<MealAddative>,
    allergens: HashSet<MealAllergen>,
    attributes: HashSet<MealAttribute>,
}


impl MealInfo {
    fn parse(desc: &str) -> Self {
        #[derive(Debug)]
        enum InfoItem {
            Addative(MealAddative),
            Allergen(MealAllergen),
            HealthRating(Rating),
        }
        use InfoItem::*;
        use MealAddative::*;
        use MealAllergen::*;
        use Rating::*;

        impl InfoItem {
            fn from_id(id: &str) -> Option<Self> {
                Some(match id {
                    "0Ampel0" => HealthRating(Green),
                    "0Ampel1" => HealthRating(Yellow),
                    "0Ampel2" => HealthRating(Red),

                    "2" => Addative(Pork),
                    "3" => Addative(Alcohol),
                    "4" => Addative(FlavourEnhancer),
                    "5" => Addative(Waxed),
                    "6" => Addative(Preserved),
                    "7" => Addative(Antioxidants),
                    "8" => Addative(Coloring),
                    "9" => Addative(Phosphate),
                    "10" => Addative(Darkened),
                    "12" => Addative(Phenylalaninsource),
                    "13" => Addative(Sweeteners),
                    "14" => Addative(SmallFishParts),
                    "16" => Addative(Caffeine),
                    "17" => Addative(Chitin),
                    "19" => Addative(Sulfur),
                    "20" => Addative(LaxativeEffect),

                    "21" => Allergen(Gluten),
                    "21a" => Allergen(Wheat),
                    "21b" => Allergen(Rye),
                    "21c" => Allergen(Barley),
                    "21d" => Allergen(Oats),
                    "21e" => Allergen(Spelt),
                    "21f" => Allergen(Hand),
                    "22" => Allergen(Crustaceans),
                    "23" => Allergen(Eggs),
                    "24" => Allergen(Fish),
                    "25" => Allergen(Peanuts),
                    "26" => Allergen(Nuts),
                    "26a" => Allergen(Almonds),
                    "26b" => Allergen(Hazelnut),
                    "26c" => Allergen(Wallnut),
                    "26d" => Allergen(Cashew),
                    "26e" => Allergen(Pecan),
                    "26f" => Allergen(Paranus),
                    "26g" => Allergen(Pistacio),
                    "26h" => Allergen(Macadamia),
                    "27" => Allergen(Cellery),
                    "28" => Allergen(Soy),
                    "29" => Allergen(Mustard),
                    "30" => Allergen(MilkProducts),
                    "31" => Allergen(Sesame),
                    "32" => Allergen(Sulfides),
                    "33" => Allergen(Lupine),
                    "34" => Allergen(Molluscs),
                    "35" => Allergen(NitriteSalt),
                    "36" => Allergen(Yeast),
                    _ => None?,
                })
            }
        }

        let mut info = MealInfo {
            env_rating: MealEnvRating {
                health: None,
                co2: None,
                h2o: None,
            },
            addatives: HashSet::new(),
            allergens: HashSet::new(),
            attributes: HashSet::new(),
        };

        for it in desc.split(",").flat_map(InfoItem::from_id) {
            #[cfg(debug_assertions)] let name = format!("{it:?}");

            let replaced = !match it {
                Addative(a) => info.addatives.insert(a),
                Allergen(a) => info.allergens.insert(a),
                HealthRating(r) => info.env_rating.health.replace(r).is_none(),
            };

            #[cfg(debug_assertions)] if replaced {
                tracing::warn!("{name:?} was inserted twice (MealInfo)");
            }
            #[cfg(not(debug_assertions))] let _ = replaced;
        }

        info
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MealAttribute {
    Vegan,
    Fairtrade,
    ClimateFood,
    Vegetarian,
    SustainableFarming,
    SustainableFishing,
    Frozen,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MealAllergen {
    Gluten,             // 21
    Wheat,              // 21a
    Rye,                // 21b
    Barley,             // 21c
    Oats,               // 21d
    Spelt,              // 21e
    Hand,               // 21f
    Crustaceans,        // 22
    Eggs,               // 23
    Fish,               // 24
    Peanuts,            // 25
    Nuts,               // 26
    Almonds,            // 26a
    Hazelnut,           // 26b
    Wallnut,            // 26c
    Cashew,             // 26d
    Pecan,              // 26e
    Paranus,            // 26f
    Pistacio,           // 26g
    Macadamia,          // 26h
    Cellery,            // 27
    Soy,                // 28
    Mustard,            // 29
    MilkProducts,       // 30
    Sesame,             // 31
    Sulfides,           // 32
    Lupine,             // 33
    Molluscs,           // 34
    NitriteSalt,        // 35
    Yeast,              // 36
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MealAddative {
    Pork,               // 2
    Alcohol,            // 3
    FlavourEnhancer,    // 4
    Waxed,              // 5
    Preserved,          // 6
    Antioxidants,       // 7
    Coloring,           // 8
    Phosphate,          // 9
    Darkened,           // 10
    Phenylalaninsource, // 12
    Sweeteners,         // 13
    SmallFishParts,     // 14
    Caffeine,           // 16
    Chitin,             // 17
    Sulfur,             // 19
    LaxativeEffect,     // 20
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MealEnvRating {
    health: Option<Rating>,
    co2: Option<Rating>,
    h2o: Option<Rating>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum Rating { Red, Yellow, Green }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MealPrice {
    students: Price,
    servants: Price,
    guests: Price,
}

#[derive(Debug, Clone)]
pub struct Price {
    eur: u32,
    cent: u8,
}

impl Serialize for Price {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer
    {
        serializer.serialize_str(&format!("{},{}", self.eur, self.cent))
    }
}

struct PriceVisitor;
impl<'de> serde::de::Visitor<'de> for PriceVisitor {
    type Value = Price;

    fn expecting(
        &self, formatter: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        write!(formatter, "some price with a comma")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where E: serde::de::Error
    {
        Price::parse(v).ok_or_else(|| {
            serde::de::Error::invalid_value(
                serde::de::Unexpected::Str(v),
                &self,
            )
        })
   }
}

impl<'de> Deserialize<'de> for Price {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de>
    { deserializer.deserialize_str(PriceVisitor) }
}

impl Price {
    fn parse(s: &str) -> Option<Self> {
        let Some((eur, cent)) = s.split_once(",") else {
            tracing::warn!("could not parse price: '{s}'");
            return None;
        };
        let (eur, cent): (u32, u32) = (
            eur.parse().ok()?,
            cent.parse().ok()?,
        );

        if cent >= 100 {
            tracing::warn!("price had over 100 cents: '{s}'");
            None
        } else {
            let cent = cent as u8;
            Some(Self { eur, cent })
        }
    }
}



