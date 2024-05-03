mod adsb;

#[tokio::test]
async fn test_all() {
    adsb::test_adsb().await;
}
