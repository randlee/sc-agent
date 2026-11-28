```json
{
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "gate": "plan",
    "item_id": "agt-m-7",
    "run_id": "plan-20261128T163953Z-agt-m-7",
    "run_path": "docs/plans/phase-m/anchor-runs/plan-20261128T163953Z-agt-m-7.json",
    "dry_run": false,
    "reused_run": false,
    "pass": false,
    "failing": [
      "ANC-010",
      "ANC-013",
      "ANC-014",
      "ANC-022",
      "ANC-025"
    ],
    "errors": [],
    "excluded": [
      {
        "anchor_id": "ANC-011",
        "reason": "no sprint in phase m carries this anchor; listed for the owner"
      },
      {
        "anchor_id": "ANC-012",
        "reason": "m-7 changes provider and model configuration rows, not the wire-format Decision implementations; agt-m-4 carries the single implementation per API shape."
      },
      {
        "anchor_id": "ANC-015",
        "reason": "m-7 changes configuration code, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this change."
      },
      {
        "anchor_id": "ANC-026",
        "reason": "m-7 defines provider and model configuration rows, not SQL tables; agt-m-8 carries core SQL ownership in sc-agent-schema."
      },
      {
        "anchor_id": "ANC-032",
        "reason": "m-7 repoints provider_keys and defines no wheel SQL tables or tool schema versions; agt-m-8 carries core-versus-tool schema ownership and versions."
      }
    ],
    "results": [
      {
        "anchor_id": "ANC-010",
        "verdict": "underspecified",
        "leaning": "satisfied",
        "min_confidence": 0.43,
        "attempts": 1,
        "explanation": "Jev was not confident (min confidence 0.43); the anchor or the bead may be underspecified.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.37,
            "probabilities": {
              "no": 0.58,
              "unclear": 0.06,
              "yes": 0.36
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.74,
            "probabilities": {
              "no": 0.83,
              "unclear": 0.05,
              "yes": 0.12
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.43,
            "probabilities": {
              "no": 0.37,
              "unclear": 0.01,
              "yes": 0.62
            }
          },
          "owns": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.61,
            "probabilities": {
              "no": 0.74,
              "unclear": 0.02,
              "yes": 0.24
            }
          },
          "paraphrase": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.68,
            "probabilities": {
              "no": 0.79,
              "unclear": 0.02,
              "yes": 0.19
            }
          },
          "partial": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.31,
            "probabilities": {
              "no": 0.54,
              "unclear": 0.01,
              "yes": 0.45
            }
          },
          "specific": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "stated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.95,
              "unclear": 0.0,
              "yes": 0.05
            }
          }
        },
        "low_questions": [
          {
            "ask": "first",
            "role": "relevance",
            "question_id": "relevant",
            "choice": "yes",
            "confidence": 0.43
          }
        ]
      },
      {
        "anchor_id": "ANC-013",
        "verdict": "underspecified",
        "leaning": "not_applicable",
        "min_confidence": 0.56,
        "attempts": 2,
        "confirmed": true,
        "explanation": "Jev was not confident (min confidence 0.56); the anchor or the bead may be underspecified.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.02,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.56,
            "probabilities": {
              "no": 0.71,
              "unclear": 0.01,
              "yes": 0.28
            }
          },
          "owns": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "paraphrase": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "partial": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.8,
            "probabilities": {
              "no": 0.87,
              "unclear": 0.0,
              "yes": 0.13
            }
          },
          "specific": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "stated": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        },
        "low_questions": [
          {
            "ask": "first",
            "role": "relevance",
            "question_id": "relevant",
            "choice": "no",
            "confidence": 0.56
          }
        ]
      },
      {
        "anchor_id": "ANC-014",
        "verdict": "underspecified",
        "leaning": "not_applicable",
        "min_confidence": 0.56,
        "attempts": 2,
        "unstable": true,
        "explanation": "Jev was not confident (min confidence 0.56); the anchor or the bead may be underspecified.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.01,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.62,
            "probabilities": {
              "no": 0.75,
              "unclear": 0.01,
              "yes": 0.24
            }
          },
          "owns": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.01,
              "yes": 0.02
            }
          },
          "paraphrase": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "partial": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.79,
            "probabilities": {
              "no": 0.86,
              "unclear": 0.0,
              "yes": 0.14
            }
          },
          "specific": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "stated": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        },
        "low_questions": [
          {
            "ask": "second",
            "role": "relevance",
            "question_id": "relevant",
            "choice": "no",
            "confidence": 0.56
          }
        ],
        "answers_second": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.01,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.56,
            "probabilities": {
              "no": 0.71,
              "unclear": 0.01,
              "yes": 0.28
            }
          }
        }
      },
      {
        "anchor_id": "ANC-022",
        "verdict": "underspecified",
        "leaning": "satisfied",
        "min_confidence": 0.13,
        "attempts": 1,
        "explanation": "Jev was not confident (min confidence 0.13); the anchor or the bead may be underspecified.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.13,
            "probabilities": {
              "no": 0.24,
              "unclear": 0.34,
              "yes": 0.42
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.43,
            "probabilities": {
              "no": 0.62,
              "unclear": 0.05,
              "yes": 0.33
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          },
          "owns": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.94,
            "probabilities": {
              "no": 0.04,
              "unclear": 0.0,
              "yes": 0.96
            }
          },
          "paraphrase": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.22,
            "probabilities": {
              "no": 0.36,
              "unclear": 0.16,
              "yes": 0.48
            }
          },
          "partial": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.77,
            "probabilities": {
              "no": 0.14,
              "unclear": 0.01,
              "yes": 0.85
            }
          },
          "specific": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.68,
            "probabilities": {
              "no": 0.79,
              "unclear": 0.06,
              "yes": 0.15
            }
          },
          "stated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.68,
            "probabilities": {
              "no": 0.79,
              "unclear": 0.03,
              "yes": 0.18
            }
          }
        },
        "low_questions": [
          {
            "ask": "first",
            "role": "compliance",
            "question_id": "addressed",
            "choice": "yes",
            "confidence": 0.13
          },
          {
            "ask": "first",
            "role": "conflict",
            "question_id": "contradicted",
            "choice": "no",
            "confidence": 0.43
          }
        ]
      },
      {
        "anchor_id": "ANC-025",
        "verdict": "underspecified",
        "leaning": "not_applicable",
        "min_confidence": 0.55,
        "attempts": 2,
        "confirmed": true,
        "explanation": "Jev was not confident (min confidence 0.55); the anchor or the bead may be underspecified.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.91,
            "probabilities": {
              "no": 0.94,
              "unclear": 0.04,
              "yes": 0.02
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.88,
            "probabilities": {
              "no": 0.92,
              "unclear": 0.01,
              "yes": 0.07
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.55,
            "probabilities": {
              "no": 0.7,
              "unclear": 0.03,
              "yes": 0.27
            }
          },
          "owns": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.89,
            "probabilities": {
              "no": 0.93,
              "unclear": 0.01,
              "yes": 0.06
            }
          },
          "paraphrase": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.01,
              "yes": 0.01
            }
          },
          "partial": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.38,
            "probabilities": {
              "no": 0.59,
              "unclear": 0.01,
              "yes": 0.4
            }
          },
          "specific": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "stated": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        },
        "low_questions": [
          {
            "ask": "first",
            "role": "relevance",
            "question_id": "relevant",
            "choice": "no",
            "confidence": 0.55
          }
        ]
      },
      {
        "anchor_id": "ANC-001",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-008",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.94,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.94,
            "probabilities": {
              "no": 0.04,
              "unclear": 0.0,
              "yes": 0.96
            }
          }
        }
      },
      {
        "anchor_id": "ANC-009",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-017",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.85,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.85,
            "probabilities": {
              "no": 0.09,
              "unclear": 0.01,
              "yes": 0.9
            }
          }
        }
      },
      {
        "anchor_id": "ANC-021",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.77,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.04,
              "unclear": 0.01,
              "yes": 0.95
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.77,
            "probabilities": {
              "no": 0.85,
              "unclear": 0.03,
              "yes": 0.12
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          }
        }
      },
      {
        "anchor_id": "ANC-002",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.8,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.95,
              "unclear": 0.01,
              "yes": 0.04
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.8,
            "probabilities": {
              "no": 0.87,
              "unclear": 0.0,
              "yes": 0.13
            }
          }
        }
      },
      {
        "anchor_id": "ANC-003",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.76,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.01,
              "yes": 0.01
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.88,
            "probabilities": {
              "no": 0.92,
              "unclear": 0.02,
              "yes": 0.06
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.76,
            "probabilities": {
              "no": 0.84,
              "unclear": 0.01,
              "yes": 0.15
            }
          }
        }
      },
      {
        "anchor_id": "ANC-004",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.85,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.01,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.85,
            "probabilities": {
              "no": 0.9,
              "unclear": 0.01,
              "yes": 0.09
            }
          }
        }
      },
      {
        "anchor_id": "ANC-005",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-006",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-016",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-018",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.67,
        "attempts": 2,
        "confirmed": true,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.01,
              "yes": 0.01
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.85,
            "probabilities": {
              "no": 0.9,
              "unclear": 0.01,
              "yes": 0.09
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.67,
            "probabilities": {
              "no": 0.78,
              "unclear": 0.01,
              "yes": 0.21
            }
          }
        }
      },
      {
        "anchor_id": "ANC-019",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        }
      },
      {
        "anchor_id": "ANC-020",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        }
      },
      {
        "anchor_id": "ANC-023",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        }
      },
      {
        "anchor_id": "ANC-024",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-027",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        }
      },
      {
        "anchor_id": "ANC-028",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.92,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.95,
              "unclear": 0.0,
              "yes": 0.05
            }
          }
        }
      },
      {
        "anchor_id": "ANC-029",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-030",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.97,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.0,
              "yes": 0.02
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
            }
          }
        }
      },
      {
        "anchor_id": "ANC-031",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.97,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.0,
              "yes": 0.02
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.0,
              "yes": 0.02
            }
          }
        }
      },
      {
        "anchor_id": "ANC-033",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 1.0,
            "probabilities": {
              "no": 1.0,
              "unclear": 0.0,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-007",
        "verdict": "deterministic_only",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Structural anchor: verified by its deterministic_check, not by Jev",
        "answers": null
      },
      {
        "anchor_id": "ANC-011",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: no sprint in phase m carries this anchor; listed for the owner.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-7",
          "reason": "no sprint in phase m carries this anchor; listed for the owner"
        }
      },
      {
        "anchor_id": "ANC-012",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-7 changes provider and model configuration rows, not the wire-format Decision implementations; agt-m-4 carries the single implementation per API shape.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-7",
          "reason": "m-7 changes provider and model configuration rows, not the wire-format Decision implementations; agt-m-4 carries the single implementation per API shape."
        }
      },
      {
        "anchor_id": "ANC-015",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-7 changes configuration code, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this change.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-7",
          "reason": "m-7 changes configuration code, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this change."
        }
      },
      {
        "anchor_id": "ANC-026",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-7 defines provider and model configuration rows, not SQL tables; agt-m-8 carries core SQL ownership in sc-agent-schema.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-7",
          "reason": "m-7 defines provider and model configuration rows, not SQL tables; agt-m-8 carries core SQL ownership in sc-agent-schema."
        }
      },
      {
        "anchor_id": "ANC-032",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-7 repoints provider_keys and defines no wheel SQL tables or tool schema versions; agt-m-8 carries core-versus-tool schema ownership and versions.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-7",
          "reason": "m-7 repoints provider_keys and defines no wheel SQL tables or tool schema versions; agt-m-8 carries core-versus-tool schema ownership and versions."
        }
      }
    ],
    "overrides_applied": [],
    "report_markdown": "## Anchor gate: plan · agt-m-7 - m-7: providers and models from configuration (change)\n\nRun `plan-20261128T163953Z-agt-m-7`, model `jev-1.14.0`, threshold 0.6. Counts: underspecified 5, satisfied 5, not_applicable 17, deterministic_only 1, excluded 5.\n\n> **agt-m-7 · ANC-010 · underspecified** Anchor: sc-agent, judge-jev and plan-anchor code must not copy and paste logic between files or tools, so the code base does not grow unnecessarily. You said (planning session; PRD D20, owner 2026-11-24, Nov 26): \"For years any copy/paste was a big red flag that I was growing the code base unnecessarily.\" Jev was not confident (min confidence 0.43); the anchor or the bead may be underspecified. (leaning: satisfied)\n>   Low: relevance `relevant` answered yes at 0.43.\n>   Triage: `owns` no (0.61), `specific` no (1.00), `stated` no (0.92), `paraphrase` no (0.68), `partial` no (0.31).\n\n> **agt-m-7 · ANC-013 · underspecified** Anchor: sc-agent must provide an LLM trait for JSON-in/JSON-out calls, with one implementation per wire format (OpenAI-compatible, Anthropic-compatible) shared by every provider speaking that format, so callers reach any LLM through one interface. You said (planning session; session, 2026-11-26, Nov 26): \"we will also need a llm-trait or similar to enable json-in/json-out llm behavior for open-ai-sdk, anthropic-sdk and compatible providers.  i.e. all providers w/ compatible api should use the same impelemntation.\" Jev was not confident (min confidence 0.56); the anchor or the bead may be underspecified. (leaning: not_applicable)\n>   Low: relevance `relevant` answered no at 0.56.\n>   Triage: `owns` no (1.00), `specific` no (1.00), `stated` no (1.00), `paraphrase` no (1.00), `partial` no (0.80).\n\n> **agt-m-7 · ANC-014 · underspecified** Anchor: When an LLM answers a decision request in place of Jev, sc-agent must send a built-in default system prompt, and the config settings decidr.luna-system-prompt and decidr.haiku-system-prompt must each override that prompt for their model, so prompts can be tested without code changes. You said (planning session; session, 2026-11-26, Nov 26): \"make sure config allows settings for decidr.luna-system-prompt and decidr.haiku-system-prompt as alternatives that can be overridden from config when we do more extensive testing w/out code changes.\" Jev was not confident (min confidence 0.56); the anchor or the bead may be underspecified. (leaning: not_applicable)\n>   Low: relevance `relevant` answered no at 0.56 (second ask).\n>   Unstable: two identical requests gave not_applicable then underspecified.\n>   Triage: `owns` no (0.96), `specific` no (1.00), `stated` no (1.00), `paraphrase` no (1.00), `partial` no (0.79).\n\n> **agt-m-7 · ANC-022 · underspecified** Anchor: sc-agent has no hard-coded row for a local model: a local model's URL and model id come only from config. You said (phase-m planning session, 2026-11-27, Nov 27): \"local models come from config. Both ip & model string.\" Jev was not confident (min confidence 0.13); the anchor or the bead may be underspecified. (leaning: satisfied)\n>   Low: compliance `addressed` answered yes at 0.13.\n>   Low: conflict `contradicted` answered no at 0.43.\n>   Triage: `owns` yes (0.94), `specific` no (0.68), `stated` no (0.68), `paraphrase` yes (0.22), `partial` yes (0.77).\n\n> **agt-m-7 · ANC-025 · underspecified** Anchor: `sc-agent-schema` contains only struct definitions, literal SQL text and convenience methods on those structs; it has no SQLite dependency and no upgrade code. You said (phase-m planning session, 2026-11-27; Rand, 2026-11-27 23:44, on this candidate: \"6. Remove upgrade\", Nov 27): \"I consider schema to hold schema (structs and literal sql text) only. Exceptions would be extension convenience methods/transforms and schema versioning/upgrade code.\" Jev was not confident (min confidence 0.55); the anchor or the bead may be underspecified. (leaning: not_applicable)\n>   Low: relevance `relevant` answered no at 0.55.\n>   Triage: `owns` no (0.89), `specific` no (1.00), `stated` no (0.98), `paraphrase` no (0.97), `partial` no (0.38).\n\n> **agt-m-7 · ANC-011 · excluded** Anchor: Every operation the decidr CLI offers must also be callable from the sc-agent Python API, so the Python API is never less flexible than the CLI. Not judged: no sprint in phase m carries this anchor; listed for the owner.\n\n> **agt-m-7 · ANC-012 · excluded** Anchor: sc-agent's decision providers must implement one decision trait, with one implementation per wire format (Jev TypeSafe systemone, OpenAI-compatible chat/completions, Anthropic-compatible Messages), and every provider that speaks a wire format must use that format's implementation, so adding a provider needs no new code or hard-coding. Not judged: m-7 changes provider and model configuration rows, not the wire-format Decision implementations; agt-m-4 carries the single implementation per API shape.\n\n> **agt-m-7 · ANC-015 · excluded** Anchor: A rule that applies to every decidr-based tool (logging, model resolution and the like) must be specified only in sc-agent's records; a derivative tool's records (judge-jev, plan-anchor and others) must not restate it and give only the tool name and the tool's own behaviour, so each rule has one source. Not judged: m-7 changes configuration code, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this change.\n\n> **agt-m-7 · ANC-026 · excluded** Anchor: Every core table's SQL is defined in `sc-agent-schema` and nowhere else. Not judged: m-7 defines provider and model configuration rows, not SQL tables; agt-m-8 carries core SQL ownership in sc-agent-schema.\n\n> **agt-m-7 · ANC-032 · excluded** Anchor: Each tool's tables carry their own schema version, independent of the core schema version; no table definition is in the wheel crate `sc-agent-py`. Not judged: m-7 repoints provider_keys and defines no wheel SQL tables or tool schema versions; agt-m-8 carries core-versus-tool schema ownership and versions."
  },
  "error": null,
  "metadata": {
    "duration_ms": 2500,
    "tool_calls": 31,
    "retry_count": 4,
    "correlation_id": "agt-m-7"
  }
}
```
