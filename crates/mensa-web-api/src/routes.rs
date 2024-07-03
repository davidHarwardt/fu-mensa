
use axum::{
    extract::{FromRef, Query, State},
    http::StatusCode,
    routing::get,
    Json,
    Router,
};

use chrono::{Days, NaiveDate, Weekday};
use mensa_meal_api::{MealDay, MealPlan};
use tokio_cron_scheduler::Job;

use crate::config::Config;

use self::data::{MealCollections, MealPlanManager};
use std::time::Instant;

mod data;
mod helpers;
use helpers::*;

#[derive(Clone, FromRef)]
struct AppState {
    meals: MealPlanManager,
    db: Option<mongodb::Database>,
}

impl AppState {
    async fn new(config: &Config) -> Self {
        let db = if let Some(db) = &config.db {
            tracing::info!("connecting to db");
            let v = connect_db(db).await;
            tracing::info!("connected to db");
            v
        } else {
            tracing::info!("no db specified");
            None
        };

        let meals = MealPlanManager::new(db.as_ref().map(MealCollections::new));

        let m = meals.clone();
        register_jobs(|shed| async move {
            // run every night at 00:01
            shed.add(Job::new_async("0 1 0 1/1 * ? *", move |uuid, _| {
                let m = m.clone();
                async move {
                    tracing::info!("fetching new data (job: {uuid:?})");
                    let start = Instant::now();
                    m.fetch_all().await;
                    let took = start.elapsed();
                    tracing::info!("fetched new data (took {took:?})");
                }.pin()
            })?).await?;

            Ok(shed)
        }).await;

        Self { meals, db }
    }
}

pub async fn make_router(config: &Config) -> Router {
    Router::new()
        .route("/api/meals", get(meals))
        .route("/api/meals/plan", get(meals_plan))
        .with_state(AppState::new(config).await)
    .fallback_service(fallback_service())
}


#[derive(Default, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum MensaRelativeDate {
    #[default] Today, Yesterday, Tomorrow,
}


#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum MensaDate {
    Relative(MensaRelativeDate),
    Weekday(Weekday),
    Date(NaiveDate),
}

impl MensaDate {
    fn as_date(self) -> Option<NaiveDate> {
        let today = chrono::Utc::now().date_naive();
        Some(match self {
            MensaDate::Relative(MensaRelativeDate::Today) => today,
            MensaDate::Relative(MensaRelativeDate::Yesterday) => today.pred_opt()?,
            MensaDate::Relative(MensaRelativeDate::Tomorrow) => today.succ_opt()?,
            MensaDate::Date(d) => d,
            MensaDate::Weekday(w) => today.week(Weekday::Mon)
                .first_day()
            .checked_add_days(Days::new(w.num_days_from_monday() as _))?,
        })
    }
}

#[derive(Debug, serde::Deserialize)]
struct MensaQuery {
    mensa: String,
    lang: Option<String>,
    day: Option<MensaDate>,
}

async fn meals_plan(
    Query(q): Query<MensaQuery>,
    State(state): State<MealPlanManager>,
) -> Result<Json<MealPlan>, (StatusCode, Json<String>)> {
    let lang = q.lang.as_ref().map(String::as_str);
    Ok(Json(state.get_plan(
        &q.mensa,
        lang,
    ).await.map_err(|_| (StatusCode::NOT_FOUND, Json(format!("plan_not_found"))))?))
}

async fn meals(
    Query(q): Query<MensaQuery>,
    State(state): State<MealPlanManager>,
) -> Result<Json<MealDay>, (StatusCode, Json<String>)> {
    let lang = q.lang.as_ref().map(String::as_str);
    let d = q.day.unwrap_or(MensaDate::Relative(MensaRelativeDate::Today));
    Ok(Json(
        state.get_day(&q.mensa, lang,
            &d.as_date().ok_or_else(|| {
                (StatusCode::BAD_REQUEST, Json(format!("invalid_date")))
            })?,
        ).await.ok_or_else(||
            (StatusCode::NOT_FOUND, Json(format!("plan_not_found")))
        )?
    ))
}

fn fallback_service() -> Router {
    Router::new()
}

