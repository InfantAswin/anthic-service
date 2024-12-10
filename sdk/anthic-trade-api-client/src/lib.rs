pub mod model;

use crate::model::*;

pub struct AnthicTradeApiClient {
    client: reqwest::Client,
    url: String,
    api_key: String,
}

impl AnthicTradeApiClient {
    pub fn new(url: String, api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            url,
            api_key,
        }
    }

    pub async fn network_status(&self) -> Result<NetworkStatusResponse, reqwest::Error> {
        let url = format!("{}/network/status", &self.url);
        let res = self.client.get(url).send().await?;
        res.json().await
    }

    pub async fn info(&self) -> Result<InfoResponse, reqwest::Error> {
        let url = format!("{}/trade/info", &self.url);
        let res = self.client.get(url).send().await?;
        res.json().await
    }

    pub async fn account_address_info(
        &self,
        address: String,
    ) -> Result<AccountAddressInfo, reqwest::Error> {
        let url = format!("{}/trade/account_addresses/{}", &self.url, address);
        let res = self.client.get(url).send().await?;
        res.json().await
    }

    pub async fn tokens(&self) -> Result<TokensResponse, reqwest::Error> {
        let url = format!("{}/trade/tokens", &self.url);
        let res = self.client.get(url).send().await?;
        res.json().await
    }

    pub async fn token_pairs(&self) -> Result<TokenPairsResponse, reqwest::Error> {
        let url = format!("{}/trade/token_pairs", &self.url);
        let res = self.client.get(url).send().await?;
        res.json().await
    }

    pub async fn accounts(&self) -> Result<AccountsResponse, reqwest::Error> {
        let url = format!("{}/trade/accounts", &self.url);
        let res = self
            .client
            .get(url)
            .header("ANTHIC-API-KEY", self.api_key.as_str())
            .send()
            .await?;
        res.json().await
    }

    pub async fn instamint_accounts(&self) -> Result<InstamintAccountsResponse, reqwest::Error> {
        let url = format!("{}/instamint/accounts", &self.url);
        let res = self
            .client
            .get(url)
            .header("ANTHIC-API-KEY", self.api_key.as_str())
            .send()
            .await?;
        res.json().await
    }

    pub async fn instamint_info(&self) -> Result<InstamintInfo, reqwest::Error> {
        let url = format!("{}/instamint/info", &self.url);
        let res = self
            .client
            .get(url)
            .send()
            .await?;
        res.json().await
    }
}
