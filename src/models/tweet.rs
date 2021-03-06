use super::settings::Settings;
use crate::db;
use egg_mode::search::{self, ResultType};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, error, fmt::Display, result};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum StatusTweet {
    Pending,
    Discarted,
    Liked,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Tweet {
    pub id: u64,
    pub text: String,
    pub url: Option<String>,
    pub screen_name: Option<String>,
    pub status: StatusTweet,
}

impl Tweet {
    pub fn save(&self) -> result::Result<(), Box<dyn error::Error>> {
        let db = db::init_db()?;

        db.write(|db| {
            if db.tweets.contains_key(&self.id) {
                return;
            }
            db.tweets.insert(self.id, self.clone());
        })
        .unwrap();

        db.save()?;

        Ok(())
    }
}

impl Display for Tweet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(screen_name) = self.screen_name.clone() {
            write!(f, "@{}\n", screen_name)?;
        }

        write!(f, "\t{}\n", self.text)?;
        if let Some(url) = self.url.clone() {
            write!(f, "link: {}\n", url)?;
        }

        Ok(())
    }
}

pub async fn search(
    token: &egg_mode::Token,
) -> Result<(), Box<dyn error::Error>> {
    let settings = Settings::load()?;
    let query = settings.search_terms;

    search::search(query.join(" "))
        .result_type(ResultType::Recent)
        .count(settings.search_count)
        .call(token)
        .await
        .iter()
        .map(|s| &s.response.statuses)
        .for_each(|statuses| {
            statuses
                .iter()
                .filter(|s| match s.favorited {
                    Some(is_favorited) => !is_favorited,
                    None => true,
                })
                .for_each(|s| new(&s).save().unwrap())
        });

    Ok(())
}

fn new(tweet: &egg_mode::tweet::Tweet) -> Tweet {
    let id = tweet.id;
    let text = tweet.text.clone();
    let mut screen_name = None;
    let mut url = None;
    let status = StatusTweet::Pending;

    if let Some(user) = tweet.user.clone() {
        screen_name = Some(user.screen_name.clone());
        url = Some(format!(
            "https://www.twitter.com/{}/status/{}",
            user.screen_name.clone(),
            id
        ));
    };

    Tweet {
        id,
        text,
        screen_name,
        url,
        status,
    }
}

fn find_by_status(
    status: Option<StatusTweet>,
) -> Result<HashMap<u64, Tweet>, Box<dyn error::Error>> {
    let db = db::init_db()?;

    let tweets = db.read(|db| match status {
        Some(status) => db
            .tweets
            .clone()
            .into_iter()
            .filter(|(_id, t)| t.status == status)
            .collect(),
        None => db.tweets.clone(),
    })?;

    Ok(tweets)
}

pub fn get_all() -> Result<HashMap<u64, Tweet>, Box<dyn error::Error>> {
    find_by_status(None)
}

pub fn get_pending() -> Result<HashMap<u64, Tweet>, Box<dyn error::Error>> {
    find_by_status(Some(StatusTweet::Pending))
}

pub fn get_discarted() -> Result<HashMap<u64, Tweet>, Box<dyn error::Error>> {
    find_by_status(Some(StatusTweet::Discarted))
}

pub fn get_liked() -> Result<HashMap<u64, Tweet>, Box<dyn error::Error>> {
    find_by_status(Some(StatusTweet::Liked))
}

fn to_decide_discard(tweet: &mut Tweet) -> Result<(), Box<dyn error::Error>> {
    let settings = Settings::load()?;
    let list = settings.black_list;

    let re = Regex::new(&list.join("|").as_str())?;

    if re.is_match(tweet.text.as_str().to_lowercase().trim()) {
        tweet.status = StatusTweet::Discarted;
    }

    Ok(())
}

async fn to_decide_like(tweet: &mut Tweet) -> Result<(), Box<dyn error::Error>> {
    let db = db::init_db()?;

    if let Tweet {
        status: StatusTweet::Discarted,
        ..
    } = &tweet
    {
        return Ok(());
    }

    let user = db.read(|db| db.user.clone())?;

    if let Some(token) = user.token {
        match egg_mode::tweet::show(tweet.id, &token).await {
            Ok(t) => {
                if let Some(is_favorited) = t.response.favorited {
                    if !is_favorited {
                        egg_mode::tweet::like(tweet.id, &token).await?;
                    }
                    tweet.status = StatusTweet::Liked;
                }
            }
            Err(e) => return Err(Box::new(e)),
        };
    }

    Ok(())
}

pub async fn processing() -> Result<(), Box<dyn error::Error>> {
    let db = db::init_db()?;
    let mut tweets = get_all()?;

    for (id, mut tweet) in tweets.clone() {
        if let Tweet {
            status: StatusTweet::Pending,
            ..
        } = &tweet
        {
            to_decide_discard(&mut tweet)?;
            to_decide_like(&mut tweet).await?;
            tweets.insert(id, tweet);
        }
    }

    db.write(|db| {
        db.tweets = tweets.clone();
    })?;

    db.save()?;

    Ok(())
}
