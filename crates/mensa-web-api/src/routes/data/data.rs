
use mensa_meal_api::{MealDay, MealPlanParseError};
use mongodb::bson::oid::ObjectId;
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum MealPlanError {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    ParsePlan(#[from] MealPlanParseError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MensaMealDay {
    pub mensa_record_id: ObjectId,
    pub meal: MealDay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MensaData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub _id: Option<ObjectId>,
    pub name: String,
    pub mensa_id: String,
    pub lang: String,
}


