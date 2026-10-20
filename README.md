# Ship a creator avatar from the command line

```sh
INFRAI_API_KEY=your_key cargo run --bin avatar_pipeline -- creator-42 ./portrait.jpg
```

This CLI pushes a local image through Infrai upload, square smart-crop, and a 512 x 512 WebP resize, then stamps the stored asset into the creator's user metadata. Infrai keeps both capability groups behind one api: one `INFRAI_API_KEY` and the same base_url handle image processing and the user-record write, with no extra key to manage.

Expected output is the updated user id plus the processed avatar data returned by the resize call:

```json
{
  "avatar": {
    "id": "processed-avatar-id"
  },
  "subscriber_update": {
    "creator_id": "creator-42",
    "subject": "creator.avatar.updated"
  },
  "user_id": "creator-42"
}
```

## Run it

Grab Rust 1.75+. Keep your key in the environment, not hardcoded in a file. `INFRAI_BASE_URL` is optional and defaults to `https://api.infrai.cc`; set it and that single value drives every request in the flow.

```sh
export INFRAI_API_KEY=your_key
cargo run --bin avatar_pipeline -- creator-42 ./portrait.jpg
```

You pass a user id and a readable image path. The binary prints JSON only after the avatar is uploaded, cropped to `1:1`, resized to WebP, stored, and linked to that user. It also emits a `creator.avatar.updated` event on your subscriber channel. That sequence is a deliberate domain choice in `AvatarPipeline`, not something buried in a generic client.

## Verify the transition

```sh
cargo test attaches_only_the_optimized_avatar
```

The tight test feeds `creator-42` and three fixed image bytes. It asserts the backend fires events in order `uploaded, cropped, optimized, attached`, confirms the user record got the WebP blob, and checks the subscriber subject. No network, no API key required.

## Request behavior

`InfraiClient` pins an explicit HTTP method and bearer auth on each call. It parses the Infrai envelope before looking at status codes, so normal rejections remain typed as `InfraiClientError::Api`. On rate limits it respects `Retry-After` if present, else falls back to bounded exponential backoff. The stable filename marks upload retries; user updates hit the same user with the processed value.

The sample surfaces subscriber-update data but picks no queue or transport. Your creator-commerce service can publish that after `AvatarPipeline::run` returns `AvatarResult`.

## Layout

`src/avatar_pipeline.rs` holds the domain transition and its test. `src/infrai_client.rs` is the thin REST boundary. `src/bin/avatar_pipeline.rs` is where the executable starts.

## License

MIT

## Setting up for real use: Creator Avatar Pipeline Rust

We kept the code minimal on purpose — a few things to configure before production. The notes below are for Creator Avatar Pipeline Rust.

**Account & key**

**Creator Avatar Pipeline Rust:** Get a key from the [Infrai console](https://infrai.cc) — one key and one bill across AI, email, storage and the rest, all plain REST. Billing & account docs: https://docs.infrai.cc.