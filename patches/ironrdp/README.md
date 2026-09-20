# IronRDP microphone patch provenance

`0001-audio-input-server.patch` applies to the exact upstream IronRDP commit
`5198cde0f73485f3fbd999ea7c49293c86b80387`. This retains the session-preemption API
used by this Hypr-RDP build instead of adopting unrelated later server changes.

It combines these upstream changes:

- [AUDIO_INPUT protocol server, PR #1945](https://github.com/Devolutions/IronRDP/pull/1945), commit `a1bd456`.
- [Server integration, PR #1946](https://github.com/Devolutions/IronRDP/pull/1946), commit `f6f6822`.
- PR #1946 follow-up fixes, commit `fe73e188ba973b548a0bde0cb8839489fcb11312`.

The original changes were authored by Greg Lamberson. The combined patch also
removes audio payload bytes from a trace event; packet length remains available
for diagnostics. No microphone sample contents should be written to that log.

IronRDP remains under its upstream licensing terms. The preparation script keeps
the complete upstream source checkout, including its license files; this patch
does not replace or remove those notices. See the [upstream repository](https://github.com/Devolutions/IronRDP).

Patch SHA-256:
`eb4929bc863f94322009dbf954f5fab8db3ecaede9388ddfa3fab819cd6ecbbd`

This is a development integration, not a claim that PR #1946 has been merged or
that all RDP clients have passed microphone interoperability testing.
