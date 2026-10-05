//! A plugin tested by playing mochid on the other end of a socket pair.
//! The "Rust SDK" page of the documentation includes this file.

// ANCHOR: test
use mochi_sdk::protocol::plugin::{CompositorState, FromPlugin, ToPlugin};
use mochi_sdk::protocol::{decode, encode};
use mochi_sdk::{ActivitySpec, ModuleCtx, ModuleEvent, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// The plugin under test: `say` shows its text on the island.
async fn plugin(mut ctx: ModuleCtx) -> Result<(), mochi_sdk::Error> {
    while let Some(event) = ctx.next_event().await {
        if let ModuleEvent::Command(command) = event {
            let text = command.args.str("text").unwrap_or("hi").to_owned();
            ctx.present(ActivitySpec::new("Say").payload(json!({ "text": text })));
            command.reply(Ok(()));
        }
    }
    Ok(())
}

#[tokio::test]
async fn say_shows_the_text() {
    let (mochid, theirs) = UnixStream::pair().unwrap();
    let (reader, mut writer) = mochid.into_split();
    let mut lines = BufReader::new(reader).lines();

    // mochid speaks first.
    let hello = ToPlugin::Hello {
        api: mochi_sdk::API,
        version: "test".into(),
        module: "say".into(),
        settings: json!({}),
        data_dir: "/tmp/say".into(),
        session_dir: "/tmp/say".into(),
        compositor: CompositorState::default(),
    };
    writer.write_all(&encode(&hello).unwrap()).await.unwrap();
    let ctx = ModuleCtx::over(theirs).await.unwrap();
    let running = tokio::spawn(plugin(ctx));

    let mut next =
        async || -> FromPlugin { decode(&lines.next_line().await.unwrap().unwrap()).unwrap() };
    assert_eq!(
        next().await,
        FromPlugin::Hello {
            api: mochi_sdk::API
        }
    );

    // `mochi ipc say say there`, as mochid hands it over.
    let command = ToPlugin::Command {
        id: 1,
        action: "say".into(),
        args: serde_json::from_value(json!({ "text": "there" })).unwrap(),
    };
    writer.write_all(&encode(&command).unwrap()).await.unwrap();

    let FromPlugin::Present { spec, .. } = next().await else {
        panic!("expected an activity");
    };
    assert_eq!(spec.payload, json!({ "text": "there" }));
    assert_eq!(
        next().await,
        FromPlugin::Reply {
            id: 1,
            output: None,
            error: None
        }
    );

    // Closing the connection ends the plugin.
    drop(writer);
    running.await.unwrap().unwrap();
}
// ANCHOR_END: test
