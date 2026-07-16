use reqwest::Client;
use serde_json::Value;

#[allow(dead_code)]
pub struct HanamiClient {
    client: Client,
    base_url: String,
    token: Option<String>,
}

#[allow(dead_code)]
impl HanamiClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
            base_url: "https://hanami.website/api".to_string(),
            token: None,
        }
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub async fn submit_score(&self, score_data: Value) -> Result<(), String> {
        // MOCK: Just log it
        println!("MOCK: Submitting score to Hanami Web: {:?}", score_data);
        // let res = self.client.post(&format!("{}/scores", self.base_url))
        //    .bearer_auth(self.token.as_ref().unwrap_or(&"".to_string()))
        //    .json(&score_data)
        //    .send().await.map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn submit_now_playing(&self, np_data: Value) -> Result<(), String> {
        // MOCK: Just log it
        println!("MOCK: Submitting now playing to Hanami Web: {:?}", np_data);
        Ok(())
    }

    pub async fn upload_map(&self, md5: &str, _file_path: &str) -> Result<(), String> {
        // MOCK: Just log it
        println!("MOCK: Uploading map {} to Hanami Web", md5);
        Ok(())
    }
}
