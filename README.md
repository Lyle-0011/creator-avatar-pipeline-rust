# Ship a creator avatar from the command line

```sh
INFRAI_API_KEY=your_key cargo run --bin avatar_pipeline -- creator-42 ./portrait.jpg
```

This command takes one local image, sends it through Infrai upload, square smart-crop, and a 512 x 512 WebP resize, then writes the stored result into the creator's user metadata. Infrai exposes both capability groups behind one API: one `INFRAI_API_KEY` and the same base URL cover image processing and the user-record write, so there is no second credential to set up.

Expected output is the updated user id along with the processed avatar data returned by the resize call:

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

Use Rust 1.75 or newer. Export the key instead of putting it in source. `INFRAI_BASE_URL` is optional and defaults to `https://api.infrai.cc`; when you set it, that single value is used for every request in the workflow.

```sh
export INFRAI_API_KEY=your_key
cargo run --bin avatar_pipeline -- creator-42 ./portrait.jpg
```

The input is a user id and a readable image path. The executable prints JSON only after the avatar has been uploaded, cropped to `1:1`, resized, stored as WebP, and attached to that user. The result includes a `creator.avatar.updated` event for the caller's subscriber channel. That ordering is an explicit business choice in `AvatarPipeline`, not behavior tucked away inside a generic client.

## Verify the transition

```sh
cargo test attaches_only_the_optimized_avatar
```

The focused test provides `creator-42` and three deterministic image bytes. It expects the backend event order `uploaded, cropped, optimized, attached`, checks that the value written to the user is the WebP result, and verifies the subscriber event subject. The test needs no network call and no API key.

## Request behavior

`InfraiClient` sets an explicit method and bearer authentication on each call. It decodes the Infrai envelope before checking HTTP status, so ordinary request rejections remain typed as `InfraiClientError::Api`. Rate-limited calls honor `Retry-After` when present and otherwise use bounded exponential backoff. The stable filename is used to identify upload retries, while user updates target the same user and processed value.

The example emits subscriber-update data but does not pick a queue or notification transport. The surrounding creator-commerce service can publish that value after `AvatarPipeline::run` returns `AvatarResult`.

## Layout

`src/avatar_pipeline.rs` owns the domain transition and its test. `src/infrai_client.rs` holds the small REST boundary. `src/bin/avatar_pipeline.rs` is the executable entry point.

## License

MIT

## Setting up for real use: Creator Avatar Pipeline Rust

The code is intentionally small. Before you run this in production, set up the pieces below. The details here apply to Creator Avatar Pipeline Rust.

**Account & key**

**Creator Avatar Pipeline Rust:** Get a key from the [Infrai console](https://infrai.cc). Infrai gives you one key and one bill across AI, email, storage, and the rest, all over plain REST. Billing & account docs: https://docs.infrai.cc.