use std::sync::Arc;

use chrono::NaiveDate;
use mongodb::{
    bson::doc,
    options::{
        FindOneAndReplaceOptions,
        ReplaceOptions, ReturnDocument
    },
    Collection,
};
use tokio::sync::RwLock;
use mensa_meal_api::{raw, MealDay, MealPlan, MealPlans};

mod data;
use data::*;

#[derive(Clone)]
pub struct MealPlanManager {
    client: reqwest::Client,
    collections: Option<MealCollections>,
    data: Arc<RwLock<MealPlans>>,
}

impl MealPlanManager {
    pub fn new(collections: Option<MealCollections>) -> Self {
        Self {
            client: reqwest::Client::new(),
            data: Arc::new(RwLock::new(MealPlans::default())),
            collections,
        }
    }

    pub async fn get_plan(
        &self, mensa: &str, lang: Option<&str>,
    ) -> Result<MealPlan, MealPlanError> {
        let data = self.data.read().await;

        if let Some(plan) = data.get(mensa, lang) {
            Ok(plan.clone())
        } else {
            drop(data);
            self.fetch_plan(mensa, lang).await
        }
    }

    pub async fn fetch_plan(
        &self,
        mensa: &str,
        lang: Option<&str>,
    ) -> Result<MealPlan, MealPlanError> {
        const DATA_URL: &str = r#"https://app2022.stw.berlin/api/getdata.php"#;

        let data: raw::ApiResult = self.client.get(DATA_URL)
            .query(&raw::ApiQuery::new(mensa, lang))
            .send().await?
        .json().await?;

        let plan = MealPlan::try_from(data)?;
        self.store_plan(mensa.into(), lang.map(ToOwned::to_owned), plan.clone()).await;

        Ok(plan)
    }

    async fn store_plan(
        &self, mensa_id: String, lang: Option<String>,
        plan: MealPlan,
    ) {
        self.data.write().await
            .insert(&mensa_id, lang.as_ref().map(|v| v.as_str()), plan.clone());

        if let Some(collections) = self.collections.clone() {
            tokio::spawn({
                async move {
                    if let Err(err) = collections.clone().store_plan(mensa_id, lang, &plan).await {
                        tracing::error!("could not store data: {err}");
                    }
                }
            });
        }
    }

    pub async fn fetch_all(&self) {
        let mensas: Vec<_> = self.data.read().await
            .mensas()
            .map(|(m, l)| (m.to_string(), l.to_string()))
        .collect();

        for (mensa, lang) in mensas {
            if let Err(err) = self.fetch_plan(&mensa, Some(&lang)).await {
                tracing::error!("could not fetch mensa {mensa} (in {lang}): {err}");
            } else {
                tracing::info!("updated plan for {mensa} in {lang}");
            }
        }
    }

    pub async fn get_day_internal(
        &self, mensa: &str, lang: Option<&str>,
        day: &NaiveDate,
    ) -> Option<MealDay> {
        self.data.read().await
            .get(mensa, lang)
            .and_then(|v| v.get_day_internal(day))
        .cloned()
    }

    pub async fn get_day(
        &self, mensa_id: &str, lang: Option<&str>,
        day: &NaiveDate,
    ) -> Option<MealDay> {
        if let Some(v) = self.get_day_internal(mensa_id, lang, day).await {
            Some(v)
        } else if let Some(collections) = &self.collections {
            collections.get_day(mensa_id, lang, day).await.ok().flatten()
        } else {
            self.fetch_plan(mensa_id, lang).await
                .ok()
            .and_then(|v| v.get_day_internal(day).cloned())
        }
    }
}

#[derive(Clone)]
pub struct MealCollections {
    meals: Collection<MensaMealDay>,
    mensas: Collection<MensaData>,
}

impl MealCollections {
    pub fn new(db: &mongodb::Database) -> Self {
        Self {
            meals: db.collection("meals"),
            mensas: db.collection("mensas"),
        }
    }

    async fn get_day(
        &self, mensa_id: &str, lang: Option<&str>,
        day: &NaiveDate,
    ) -> mongodb::error::Result<Option<MealDay>> {
        let Some(mensa_id) = self.mensas.find_one(doc! {
            "id": mensa_id,
            "lang": lang,
        }, None).await?.and_then(|v| v._id) else {
            return Ok(None);
        };

        Ok(self.meals.find_one(doc! {
            "mensa_record_id": mensa_id,
            "meal": {
                "date": day.to_string(),
            },
        }, None).await?.map(|v| v.meal))
    }

    async fn store_plan(
        &self, mensa_id: String, lang: Option<String>,
        plan: &MealPlan,
    ) -> mongodb::error::Result<()> {
        let lang = lang.unwrap_or_else(|| format!("en"));
        let mensa = self.mensas.find_one_and_replace(doc! {
            "mensa_id": &mensa_id,
            "lang": &lang,
        }, MensaData {
            _id: None,
            mensa_id, lang,
            name: plan.mensa().into(),
        }, Some(
            FindOneAndReplaceOptions::builder()
                .upsert(true)
                .return_document(ReturnDocument::After)
            .build()
        )).await?.expect("upsert should always create");

        let Some(mensa_record_id) = mensa._id else {
            tracing::error!("mensa does not have _id, could not store plan");
            return Ok(())
        };

        for meal in plan.days() {
            self.meals.replace_one(doc! {
                "mensa_record_id": mensa_record_id,
                "meal.date": meal.date.to_string(),
            }, MensaMealDay {
                mensa_record_id,
                meal: meal.clone(),
            }, ReplaceOptions::builder()
                .upsert(true)
            .build()).await?;
        }

        Ok(())
    }
}


