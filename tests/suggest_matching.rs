//! End-to-end matching and ranking rules, driven through a real gRPC session
//! against `FstEngine` built from a corpus with known popularity weights.
//!
//! Matching rules under test:
//! - Prefix: the query is a prefix of the suggestion ("bol" -> "bolo de mandioca").
//! - Partial: the query is a prefix of any word after the first
//!   ("mandi" -> "bolo de mandioca"); mid-word substrings never match ("dioca").
//! - Fuzzy: the query matches a word prefix within an edit distance that grows
//!   with its length: up to 3 chars none, 4-7 chars 1 edit, 8+ chars 2 edits
//!   (insertion, deletion, substitution or transposition).
//!
//! Ranking: prefix matches first, then partial, then fuzzy; inside each group,
//! most popular first. Each suggestion appears once, in its best group.
//!
//! Every test is ignored until docs/adr/0006 replaces the FST stubs; run them
//! with `cargo test --test suggest_matching -- --ignored`.

#![allow(
    clippy::unwrap_used,
    reason = "test helpers: a failure must abort the test"
)]

mod common;

use cisa::{
    autocomplete::{FstEngine, IndexEntry, build_index},
    proto::{
        ClientMessage, SuggestRequest, autocomplete_service_client::AutocompleteServiceClient,
        client_message, server_message,
    },
};
use rstest::rstest;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

/// Server default (`SuggestLimits::default_limit`) is requested with 0.
const DEFAULT_LIMIT: u32 = 0;

/// (query, popularity weight). Weights are distinct so every expected order is unambiguous.
const CORPUS: &[(&str, u64)] = &[
    ("sol de inverno", 2_000),
    ("bolo de chocolate", 1_000),
    ("bolo de mandioca", 900),
    ("mandioca frita", 800),
    ("bolo de cenoura", 700),
    ("bolinho de chuva", 500),
    ("mandioca cozida", 400),
    ("bolo de fubá", 300),
    ("farofa de mandioca", 200),
];

fn corpus_engine() -> FstEngine {
    let entries: Vec<IndexEntry> = CORPUS
        .iter()
        .map(|&(text, weight)| IndexEntry {
            text: text.to_owned(),
            weight,
        })
        .collect();
    FstEngine::from_bytes(build_index(&entries).unwrap()).unwrap()
}

fn suggest_message(query: &str, limit: u32) -> ClientMessage {
    let request = SuggestRequest {
        query: query.to_owned(),
        limit,
        ..SuggestRequest::default()
    };
    ClientMessage {
        request_id: 1,
        payload: Some(client_message::Payload::Suggest(request)),
    }
}

/// Sends one suggest request over a fresh gRPC session and returns the suggestion texts.
async fn suggest_texts(query: &str, limit: u32) -> Vec<String> {
    let server = common::spawn_server(corpus_engine()).await;
    let mut client = AutocompleteServiceClient::connect(format!("http://{}", server.addr))
        .await
        .unwrap();
    // The sender stays alive until the reply arrives: closing the client stream ends the session.
    let (requests, outbound) = mpsc::channel(1);
    requests.send(suggest_message(query, limit)).await.unwrap();
    let mut responses = client
        .session(ReceiverStream::new(outbound))
        .await
        .unwrap()
        .into_inner();
    let reply = responses.message().await.unwrap().unwrap();
    let Some(server_message::Payload::Result(result)) = reply.payload else {
        panic!("expected a result payload, got {:?}", reply.payload);
    };
    result.suggestions.into_iter().map(|s| s.text).collect()
}

#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_phrase_prefix_returns_completions_starting_with_it() {
    // Act
    let texts = suggest_texts("bol", DEFAULT_LIMIT).await;

    // Assert
    assert!(texts.contains(&"bolo de mandioca".to_owned()), "{texts:?}");
    assert!(texts.iter().all(|t| t.starts_with("bol")), "{texts:?}");
}

#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_word_prefix_mid_phrase_returns_partial_match() {
    // Act
    let texts = suggest_texts("mandi", DEFAULT_LIMIT).await;

    // Assert
    assert!(texts.contains(&"bolo de mandioca".to_owned()), "{texts:?}");
}

#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_misspelled_word_returns_fuzzy_matches_by_popularity() {
    // Act: "mendioca" has 8 chars, so up to 2 edits; it is 1 away from "mandioca".
    let texts = suggest_texts("mendioca", DEFAULT_LIMIT).await;

    // Assert
    assert_eq!(
        texts,
        [
            "bolo de mandioca",
            "mandioca frita",
            "mandioca cozida",
            "farofa de mandioca",
        ]
    );
}

#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_phrase_prefix_returns_most_popular_first() {
    // Act: "bol" has 3 chars, so no fuzzy; the more popular "sol de inverno" stays out.
    let texts = suggest_texts("bol", DEFAULT_LIMIT).await;

    // Assert
    assert_eq!(
        texts,
        [
            "bolo de chocolate",
            "bolo de mandioca",
            "bolo de cenoura",
            "bolinho de chuva",
            "bolo de fubá",
        ]
    );
}

#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_limit_below_matches_returns_top_popular() {
    // Act
    let texts = suggest_texts("bol", 2).await;

    // Assert
    assert_eq!(texts, ["bolo de chocolate", "bolo de mandioca"]);
}

#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_prefix_and_partial_matches_ranks_prefix_group_first() {
    // Act: "bolo de mandioca" (900) is more popular but only a partial match.
    let texts = suggest_texts("mandi", DEFAULT_LIMIT).await;

    // Assert
    assert_eq!(
        texts,
        [
            "mandioca frita",
            "mandioca cozida",
            "bolo de mandioca",
            "farofa de mandioca",
        ]
    );
}

#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_prefix_and_fuzzy_matches_ranks_fuzzy_group_last() {
    // Act: "bolinho de chuva" (500) is 1 edit away ("boli") and ranks below
    // the exact prefix "bolo de fubá" (300).
    let texts = suggest_texts("bolo", DEFAULT_LIMIT).await;

    // Assert
    assert_eq!(
        texts,
        [
            "bolo de chocolate",
            "bolo de mandioca",
            "bolo de cenoura",
            "bolo de fubá",
            "bolinho de chuva",
        ]
    );
}

#[rstest]
#[case::mid_word_substring("dioca")]
#[case::typo_in_short_term_disables_fuzzy("sok")]
#[case::two_edits_in_medium_term("bulu")]
#[tokio::test]
#[ignore = "pending FST engine (docs/adr/0006)"]
async fn suggest_query_outside_matching_rules_returns_empty(#[case] query: &str) {
    // Act
    let texts = suggest_texts(query, DEFAULT_LIMIT).await;

    // Assert
    assert_eq!(texts, Vec::<String>::new());
}
