| Case | Group | B-raw | B-fp static | B-fp regen | TrustInfra | TrustInfra checks failed |
|---|---|---|---|---|---|---|
| untouched copy | false alarm | ✗ | ✗ | ✗ | ✗ | verified: none |
| manifest re-formatted | false alarm | ✗ | ✓ | ✗ | ✗ | verified: none |
| artifact whitespace | byte change | ✗ | ✓ | ✗ | ✓ | tampered: artifact:results |
| edit results | artifact edit | ✗ | ✓ | ✗ | ✓ | tampered: artifact:results |
| edit log headline only | artifact edit | ✓ | ✓ | ✗ | ✓ | tampered: artifact:inspect_log |
| edit log consistently | artifact edit | ✗ | ✓ | ✗ | ✓ | tampered: artifact:inspect_log, artifact:results |
| edit scorer code | artifact edit | ✗ | ✓ | ✗ | ✓ | tampered: artifact:scorer_code |
| edit dataset | artifact edit | ✗ | ✓ | ✗ | ✓ | tampered: artifact:dataset |
| delete artifact | artifact edit | ✗ | ✓ | ✗ | ✓ | tampered: artifact:results |
| add unlisted file | artifact edit | ✗ | ✓ | ✗ | ✓ | tampered: unlisted_files |
| edit manifest model | manifest edit | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| edit and rehash | manifest edit | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| attacker re-signs | manifest edit | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| remove signature | manifest edit | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| add unknown field | manifest edit | ✗ | ✓ | ✗ | ✓ | unverifiable: schema, signature |
| unsafe artifact path | manifest edit | ✗ | ✓ | ✗ | ✓ | tampered: path_safety, signature |
| infra edit, real key | insider | ✗ | ✓ | ✗ | ✓ | tampered: infra_diff |
| D1: as run | drift D1 | ✗ | ✗ | ✗ | ✓ | tampered: infra_diff |
| D1: hide drift | drift D1 | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| D1: hide drift, attacker re-signs | drift D1 | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| D1: insider signs a lie | drift D1 | ✗ | ✓ | ✗ | ✗ | verified: none |
| D2: as run | drift D2 | ✗ | ✗ | ✗ | ✓ | tampered: infra_diff |
| D2: hide drift | drift D2 | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| D2: hide drift, attacker re-signs | drift D2 | ✗ | ✓ | ✗ | ✓ | tampered: signature |
| D2: insider signs a lie | drift D2 | ✗ | ✓ | ✗ | ✗ | verified: none |
| D7: as run | drift D7 | ✓ | ✗ | ✗ | ✓ | tampered: infra_diff |
| D7: hide drift | drift D7 | ✓ | ✓ | ✗ | ✓ | tampered: signature |
| D7: hide drift, attacker re-signs | drift D7 | ✓ | ✓ | ✗ | ✓ | tampered: signature |
| D7: insider signs a lie | drift D7 | ✓ | ✓ | ✗ | ✗ | verified: none |

✓ = noticed, ✗ = not noticed, (!) = differs from the expectation written before the run.

No expectation mismatches.
