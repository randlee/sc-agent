```json
{
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "gate": "plan",
    "item_id": "agt-m-6",
    "run_id": "plan-20261128T164001Z-agt-m-6",
    "run_path": null,
    "dry_run": false,
    "reused_run": false,
    "pass": true,
    "valid": true,
    "failing": [],
    "errors": [],
    "excluded": [
      {
        "anchor_id": "ANC-001",
        "reason": "m-6 removes the old shell and rewires imports without changing the model table or jev resolution; agt-m-7 carries model-default configuration."
      },
      {
        "anchor_id": "ANC-011",
        "reason": "no sprint in phase m carries this anchor; listed for the owner"
      },
      {
        "anchor_id": "ANC-015",
        "reason": "m-6 changes crate imports, workspace membership and release inputs, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this move."
      },
      {
        "anchor_id": "ANC-032",
        "reason": "m-6 changes wheel imports only and defines no SQL tables or schema versions; agt-m-8 carries core-versus-tool schema ownership and versions."
      }
    ],
    "results": [
      {
        "anchor_id": "ANC-010",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 1.0,
            "probabilities": {
              "no": 0.0,
              "unclear": 0.0,
              "yes": 1.0
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-017",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.86,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.92,
            "probabilities": {
              "no": 0.95,
              "unclear": 0.02,
              "yes": 0.03
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
            "confidence": 0.86,
            "probabilities": {
              "no": 0.09,
              "unclear": 0.0,
              "yes": 0.91
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-020",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.79,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
        "explanation": "The directive is carried forward.",
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
            "choice": "yes",
            "confidence": 0.79,
            "probabilities": {
              "no": 0.12,
              "unclear": 0.02,
              "yes": 0.86
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-021",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.88,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.94,
            "probabilities": {
              "no": 0.03,
              "unclear": 0.01,
              "yes": 0.96
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
            "choice": "yes",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.01,
              "unclear": 0.0,
              "yes": 0.99
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-002",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-003",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.83,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.89,
            "probabilities": {
              "no": 0.93,
              "unclear": 0.04,
              "yes": 0.03
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.83,
            "probabilities": {
              "no": 0.89,
              "unclear": 0.05,
              "yes": 0.06
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.0,
              "yes": 0.03
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-004",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
              "unclear": 0.01,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-005",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.77,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.77,
            "probabilities": {
              "no": 0.85,
              "unclear": 0.02,
              "yes": 0.13
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-006",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-008",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-009",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.85,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.85,
            "probabilities": {
              "no": 0.9,
              "unclear": 0.09,
              "yes": 0.01
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-012",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.97,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
              "unclear": 0.02,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-013",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.94,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.94,
            "probabilities": {
              "no": 0.96,
              "unclear": 0.03,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.01,
              "yes": 0.01
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-014",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-016",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.91,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.91,
            "probabilities": {
              "no": 0.94,
              "unclear": 0.06,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-018",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.82,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.82,
            "probabilities": {
              "no": 0.88,
              "unclear": 0.03,
              "yes": 0.09
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.86,
            "probabilities": {
              "no": 0.91,
              "unclear": 0.0,
              "yes": 0.09
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-019",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.82,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.82,
            "probabilities": {
              "no": 0.88,
              "unclear": 0.01,
              "yes": 0.11
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-022",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.97,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.0,
              "yes": 0.02
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-023",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.85,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
              "unclear": 0.01,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.85,
            "probabilities": {
              "no": 0.9,
              "unclear": 0.0,
              "yes": 0.1
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-024",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.91,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.91,
            "probabilities": {
              "no": 0.94,
              "unclear": 0.01,
              "yes": 0.05
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-025",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-026",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-027",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-028",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-029",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.98,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
              "unclear": 0.01,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-030",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 1.0,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-031",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.96,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.0,
              "yes": 0.03
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-033",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.97,
        "attempts": 1,
        "confirmed": null,
        "unstable": null,
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
            "confidence": 0.97,
            "probabilities": {
              "no": 0.98,
              "unclear": 0.0,
              "yes": 0.02
            }
          }
        },
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-007",
        "verdict": "deterministic_only",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "confirmed": null,
        "unstable": null,
        "explanation": "Structural anchor: verified by its deterministic_check, not by Jev",
        "answers": null,
        "low_questions": null,
        "answers_second": null,
        "exclusion": null
      },
      {
        "anchor_id": "ANC-001",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "confirmed": null,
        "unstable": null,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-6 removes the old shell and rewires imports without changing the model table or jev resolution; agt-m-7 carries model-default configuration.",
        "answers": null,
        "low_questions": null,
        "answers_second": null,
        "exclusion": {
          "item": "agt-m-6",
          "reason": "m-6 removes the old shell and rewires imports without changing the model table or jev resolution; agt-m-7 carries model-default configuration."
        }
      },
      {
        "anchor_id": "ANC-011",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "confirmed": null,
        "unstable": null,
        "explanation": "Excluded from this bead by the anchor's exclude list: no sprint in phase m carries this anchor; listed for the owner.",
        "answers": null,
        "low_questions": null,
        "answers_second": null,
        "exclusion": {
          "item": "agt-m-6",
          "reason": "no sprint in phase m carries this anchor; listed for the owner"
        }
      },
      {
        "anchor_id": "ANC-015",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "confirmed": null,
        "unstable": null,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-6 changes crate imports, workspace membership and release inputs, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this move.",
        "answers": null,
        "low_questions": null,
        "answers_second": null,
        "exclusion": {
          "item": "agt-m-6",
          "reason": "m-6 changes crate imports, workspace membership and release inputs, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this move."
        }
      },
      {
        "anchor_id": "ANC-032",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "confirmed": null,
        "unstable": null,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-6 changes wheel imports only and defines no SQL tables or schema versions; agt-m-8 carries core-versus-tool schema ownership and versions.",
        "answers": null,
        "low_questions": null,
        "answers_second": null,
        "exclusion": {
          "item": "agt-m-6",
          "reason": "m-6 changes wheel imports only and defines no SQL tables or schema versions; agt-m-8 carries core-versus-tool schema ownership and versions."
        }
      }
    ],
    "overrides_applied": [
      "ANC-010",
      "ANC-020",
      "ANC-021",
      "ANC-002",
      "ANC-003",
      "ANC-004",
      "ANC-006",
      "ANC-008",
      "ANC-009",
      "ANC-012",
      "ANC-013",
      "ANC-014",
      "ANC-016",
      "ANC-018",
      "ANC-019",
      "ANC-022",
      "ANC-023",
      "ANC-025",
      "ANC-026",
      "ANC-027",
      "ANC-028",
      "ANC-029",
      "ANC-030",
      "ANC-031",
      "ANC-033"
    ],
    "report_markdown": "## Anchor gate: plan · agt-m-6 - m-6: old `sc-agent` crate deleted; CLI and wheel on the new crates\n\nRun `plan-20261128T164001Z-agt-m-6`, model `jev-1.14.0`, threshold 0.6. Counts: satisfied 4, not_applicable 24, deterministic_only 1, excluded 4.\n\n> **agt-m-6 · ANC-001 · excluded** Anchor: The decidr model name `jev` must select the newest Jev model in sc-agent's model table unless a config file sets another default, so callers get the latest Jev without a code change. Not judged: m-6 removes the old shell and rewires imports without changing the model table or jev resolution; agt-m-7 carries model-default configuration.\n\n> **agt-m-6 · ANC-011 · excluded** Anchor: Every operation the decidr CLI offers must also be callable from the sc-agent Python API, so the Python API is never less flexible than the CLI. Not judged: no sprint in phase m carries this anchor; listed for the owner.\n\n> **agt-m-6 · ANC-015 · excluded** Anchor: A rule that applies to every decidr-based tool (logging, model resolution and the like) must be specified only in sc-agent's records; a derivative tool's records (judge-jev, plan-anchor and others) must not restate it and give only the tool name and the tool's own behaviour, so each rule has one source. Not judged: m-6 changes crate imports, workspace membership and release inputs, not derivative-tool REQ/NFR/ADR records; no record-editing sprint is carried by this move.\n\n> **agt-m-6 · ANC-032 · excluded** Anchor: Each tool's tables carry their own schema version, independent of the core schema version; no table definition is in the wheel crate `sc-agent-py`. Not judged: m-6 changes wheel imports only and defines no SQL tables or schema versions; agt-m-8 carries core-versus-tool schema ownership and versions.\n\nAll judged anchors are satisfied or not applicable.\n\n"
  },
  "error": null,
  "metadata": {
    "duration_ms": 9000,
    "tool_calls": 36,
    "retry_count": 0,
    "correlation_id": "agt-m-6"
  }
}
```
