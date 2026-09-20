# Performance notes

## Codec negotiation

A configured `--egfx-codec avc420` is a preference, not proof of the active
codec. A client advertising `AVC_DISABLED` cannot receive AVC420 or AVC444;
this build uses ClearCodec for that session. Check the negotiation log.
The flag is defined by [MS-RDPEGFX](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-rdpegfx/be5ea8da-44db-478d-b55c-d42d82f11d26).

The pinned ClearCodec encoder uses lossless BGR run-length residual coding.
Gradients, photos and animated regions can generate large frames. H.264
`--quality`, `--bitrate` and VA-API settings do not tune this encoder.
Do not force H.264 against the capabilities sent by the client.

## Frame pacing

The Omarchy integration defaults to a 60 FPS limit and two unacknowledged
frames. This is a limit, not a guaranteed delivered frame rate. One queued
frame can reduce buffering but also reduce throughput on higher-latency links.
Resolution, changed image complexity and available bandwidth still matter.
Reducing the FPS limit reduces offered load; it does not shrink an individual
large frame.

## Manual microphone regression across codecs

The synthetic microphone test also accepts `MIC_TEST_GFX` and `MIC_TEST_DIR`.
For a local test server, use `MIC_TEST_GFX=/gfx:AVC420:off,AVC444:off` to exercise
a no-AVC client, and confirm ClearCodec selection in the server log. The default
is `/gfx:AVC420`. The test sends a generated tone, not the physical microphone.

A working microphone or an increased FPS limit is not a latency benchmark.
Compare server input-lock stalls and frame transmission times under the same
client, resolution and network conditions before claiming an improvement.

## Server state lock during event writes

The custom dependency patch encodes a bounded event batch, reserves the socket
writer to retain wire order, then releases the server-state mutex before the
network flush. The next batch is not staged until the previous flush finishes.
A batch is limited to 100 events and 64 MiB; exceeding the byte limit fails the
connection. Handlers that need to send a response may still wait for the writer.
This change does not increase bandwidth or change codec negotiation.

Four server regressions passed: input progress and wire/disconnect ordering,
socket-error propagation, staging size limit, and flushing the valid prefix
before returning a later encoding error. In one synthetic test, the socket was
blocked for 251 ms and FastPath keyboard processing completed in 15 microseconds
while it remained blocked. This is not an end-to-end user latency benchmark.

A local FreeRDP connection advertising AVC_DISABLED negotiated ClearCodec and
passed the 48 kHz mono synthetic microphone test (143360 samples; 440 Hz tone
fraction 0.986). Physical client animation smoothness after this change remains
to be measured on the user's network.
