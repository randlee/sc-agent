```json
{
  "success": true,
  "canceled": false,
  "aborted_by": null,
  "data": {
    "gate": "plan",
    "item_id": "agt-m-4",
    "run_id": "plan-20261128T163945Z-agt-m-4",
    "run_path": "docs/plans/phase-m/anchor-runs/plan-20261128T163945Z-agt-m-4.json",
    "dry_run": false,
    "reused_run": false,
    "pass": false,
    "failing": [
      "ANC-014"
    ],
    "errors": [],
    "excluded": [
      "ANC-001",
      "ANC-015",
      "ANC-022",
      "ANC-030"
    ],
    "results": [
      {
        "anchor_id": "ANC-014",
        "verdict": "underspecified",
        "leaning": "satisfied",
        "min_confidence": 0.5,
        "attempts": 2,
        "unstable": true,
        "explanation": "Jev was not confident (min confidence 0.50); the anchor or the bead may be underspecified.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.5,
            "probabilities": {
              "no": 0.11,
              "unclear": 0.22,
              "yes": 0.67
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.67,
            "probabilities": {
              "no": 0.78,
              "unclear": 0.05,
              "yes": 0.17
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
          },
          "owns": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.53,
            "probabilities": {
              "no": 0.31,
              "unclear": 0.0,
              "yes": 0.69
            }
          },
          "paraphrase": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.53,
            "probabilities": {
              "no": 0.15,
              "unclear": 0.16,
              "yes": 0.69
            }
          },
          "partial": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.89,
            "probabilities": {
              "no": 0.07,
              "unclear": 0.0,
              "yes": 0.93
            }
          },
          "specific": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.83,
            "probabilities": {
              "no": 0.89,
              "unclear": 0.02,
              "yes": 0.09
            }
          },
          "stated": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.62,
            "probabilities": {
              "no": 0.24,
              "unclear": 0.01,
              "yes": 0.75
            }
          }
        },
        "low_questions": [
          {
            "ask": "first",
            "role": "compliance",
            "question_id": "addressed",
            "choice": "yes",
            "confidence": 0.5
          }
        ],
        "answers_second": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.72,
            "probabilities": {
              "no": 0.05,
              "unclear": 0.14,
              "yes": 0.81
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.82,
            "probabilities": {
              "no": 0.88,
              "unclear": 0.04,
              "yes": 0.08
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
        "to_fix": {
          "where": "Each LLM shape sends the built-in `DEFAULT_SYSTEM_PROMPT` unless the runner passes a configured override in `Route::system_prompt` (ANC-014); the `decidr.luna-system-prompt` and `decidr.haiku-system-prompt` lookup moves to the runner in m-5.",
          "change": "State that m-5 reads `decidr.luna-system-prompt` and `decidr.haiku-system-prompt` from config and passes each value as the override for its corresponding model."
        }
      },
      {
        "anchor_id": "ANC-008",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.89,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.89,
            "probabilities": {
              "no": 0.05,
              "unclear": 0.02,
              "yes": 0.93
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
            "confidence": 0.96,
            "probabilities": {
              "no": 0.03,
              "unclear": 0.0,
              "yes": 0.97
            }
          }
        }
      },
      {
        "anchor_id": "ANC-010",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.96,
        "attempts": 1,
        "explanation": "The directive is carried forward.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "yes",
            "confidence": 0.91,
            "probabilities": {
              "no": 0.05,
              "unclear": 0.01,
              "yes": 0.94
            }
          },
          "contradicted": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.01,
              "yes": 0.02
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
        "anchor_id": "ANC-012",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.94,
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
        "anchor_id": "ANC-013",
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
        "anchor_id": "ANC-021",
        "verdict": "satisfied",
        "leaning": "satisfied",
        "min_confidence": 0.83,
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
            "confidence": 0.83,
            "probabilities": {
              "no": 0.89,
              "unclear": 0.02,
              "yes": 0.09
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
        "anchor_id": "ANC-002",
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
        "anchor_id": "ANC-003",
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
        "anchor_id": "ANC-004",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.94,
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
            "confidence": 0.94,
            "probabilities": {
              "no": 0.96,
              "unclear": 0.04,
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
        "anchor_id": "ANC-005",
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
        "anchor_id": "ANC-009",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.85,
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
            "confidence": 0.85,
            "probabilities": {
              "no": 0.9,
              "unclear": 0.1,
              "yes": 0.0
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.88,
            "probabilities": {
              "no": 0.92,
              "unclear": 0.02,
              "yes": 0.06
            }
          }
        }
      },
      {
        "anchor_id": "ANC-011",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.86,
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
            "confidence": 0.94,
            "probabilities": {
              "no": 0.96,
              "unclear": 0.01,
              "yes": 0.03
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.86,
            "probabilities": {
              "no": 0.91,
              "unclear": 0.01,
              "yes": 0.08
            }
          }
        }
      },
      {
        "anchor_id": "ANC-016",
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
              "unclear": 0.02,
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
        "anchor_id": "ANC-017",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.92,
        "attempts": 1,
        "explanation": "The anchor does not govern this bead.",
        "answers": {
          "addressed": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.98,
            "probabilities": {
              "no": 0.99,
              "unclear": 0.0,
              "yes": 0.01
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
              "unclear": 0.01,
              "yes": 0.04
            }
          }
        }
      },
      {
        "anchor_id": "ANC-018",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.89,
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
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.02,
              "yes": 0.01
            }
          },
          "relevant": {
            "type": "choice",
            "choice": "no",
            "confidence": 0.89,
            "probabilities": {
              "no": 0.93,
              "unclear": 0.0,
              "yes": 0.07
            }
          }
        }
      },
      {
        "anchor_id": "ANC-019",
        "verdict": "not_applicable",
        "leaning": "not_applicable",
        "min_confidence": 0.96,
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
            "confidence": 0.96,
            "probabilities": {
              "no": 0.97,
              "unclear": 0.0,
              "yes": 0.03
            }
          }
        }
      },
      {
        "anchor_id": "ANC-020",
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
        "anchor_id": "ANC-023",
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
        "anchor_id": "ANC-025",
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
        "anchor_id": "ANC-026",
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
        "anchor_id": "ANC-028",
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
        "anchor_id": "ANC-031",
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
        "anchor_id": "ANC-032",
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
              "unclear": 0.01,
              "yes": 0.0
            }
          }
        }
      },
      {
        "anchor_id": "ANC-033",
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
        "anchor_id": "ANC-007",
        "verdict": "deterministic_only",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Structural anchor: verified by its deterministic_check, not by Jev",
        "answers": null
      },
      {
        "anchor_id": "ANC-001",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-4 moves the API-shape implementations and changes no model table, model resolution or jev alias; agt-m-7 carries ANC-001.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-4",
          "reason": "m-4 moves the API-shape implementations and changes no model table, model resolution or jev alias; agt-m-7 carries ANC-001."
        }
      },
      {
        "anchor_id": "ANC-015",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: agt-m-4 moves sc-agent transport, types, and provider implementations but changes no REQ, NFR, or ADR records for derivative tools; derivative-record edits are not assigned to agt-m-4.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-4",
          "reason": "agt-m-4 moves sc-agent transport, types, and provider implementations but changes no REQ, NFR, or ADR records for derivative tools; derivative-record edits are not assigned to agt-m-4."
        }
      },
      {
        "anchor_id": "ANC-022",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: m-4 moves the API-shape implementations and adds no model row, local-model URL or model id; agt-m-7 carries local-model configuration.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-4",
          "reason": "m-4 moves the API-shape implementations and adds no model row, local-model URL or model id; agt-m-7 carries local-model configuration."
        }
      },
      {
        "anchor_id": "ANC-030",
        "verdict": "excluded",
        "leaning": null,
        "min_confidence": null,
        "attempts": 0,
        "explanation": "Excluded from this bead by the anchor's exclude list: agt-m-4 moves the existing resend and holdoff behavior unchanged; agt-m-9 owns parameterized waits, limits, and busy/quota rules.",
        "answers": null,
        "exclusion": {
          "item": "agt-m-4",
          "reason": "agt-m-4 moves the existing resend and holdoff behavior unchanged; agt-m-9 owns parameterized waits, limits, and busy/quota rules."
        }
      }
    ],
    "overrides_applied": [
      "ANC-014",
      "ANC-008",
      "ANC-010",
      "ANC-012",
      "ANC-013",
      "ANC-021",
      "ANC-002",
      "ANC-003",
      "ANC-004",
      "ANC-006",
      "ANC-009",
      "ANC-011",
      "ANC-016",
      "ANC-018",
      "ANC-019",
      "ANC-020",
      "ANC-023",
      "ANC-025",
      "ANC-026",
      "ANC-027",
      "ANC-028",
      "ANC-029",
      "ANC-031",
      "ANC-032",
      "ANC-033"
    ],
    "report_markdown": "## Anchor gate: plan · agt-m-4 - m-4: `sc-agent-http` and the three API-shape crates\n\nRun `plan-20261128T163945Z-agt-m-4`, model `jev-1.14.0`, threshold 0.6. Counts: underspecified 1, satisfied 5, not_applicable 22, deterministic_only 1, excluded 4.\n\n> **agt-m-4 · ANC-014 · underspecified** Anchor: When an LLM answers a decision request in place of Jev, sc-agent must send a built-in default system prompt, and the config settings decidr.luna-system-prompt and decidr.haiku-system-prompt must each override that prompt for their model, so prompts can be tested without code changes. You said (planning session; session, 2026-11-26, Nov 26): \"make sure config allows settings for decidr.luna-system-prompt and decidr.haiku-system-prompt as alternatives that can be overridden from config when we do more extensive testing w/out code changes.\" Jev was not confident (min confidence 0.50); the anchor or the bead may be underspecified. (leaning: satisfied)\n>   Low: compliance `addressed` answered yes at 0.50.\n>   Unstable: two identical requests gave underspecified then satisfied.\n>   Triage: `owns` yes (0.53), `specific` no (0.83), `stated` yes (0.62), `paraphrase` yes (0.53), `partial` yes (0.89).\n\n> **agt-m-4 · ANC-001 · excluded** Anchor: The decidr model name `jev` must select the newest Jev model in sc-agent's model table unless a config file sets another default, so callers get the latest Jev without a code change. Not judged: m-4 moves the API-shape implementations and changes no model table, model resolution or jev alias; agt-m-7 carries ANC-001.\n\n> **agt-m-4 · ANC-015 · excluded** Anchor: A rule that applies to every decidr-based tool (logging, model resolution and the like) must be specified only in sc-agent's records; a derivative tool's records (judge-jev, plan-anchor and others) must not restate it and give only the tool name and the tool's own behaviour, so each rule has one source. Not judged: agt-m-4 moves sc-agent transport, types, and provider implementations but changes no REQ, NFR, or ADR records for derivative tools; derivative-record edits are not assigned to agt-m-4.\n\n> **agt-m-4 · ANC-022 · excluded** Anchor: sc-agent has no hard-coded row for a local model: a local model's URL and model id come only from config. Not judged: m-4 moves the API-shape implementations and adds no model row, local-model URL or model id; agt-m-7 carries local-model configuration.\n\n> **agt-m-4 · ANC-030 · excluded** Anchor: Retry and holdoff code must take its waits, limits and busy and quota rules as parameters from the calling layer and contain no provider name, host or status-code table. Not judged: agt-m-4 moves the existing resend and holdoff behavior unchanged; agt-m-9 owns parameterized waits, limits, and busy/quota rules.\n\n"
  },
  "error": null,
  "metadata": {
    "duration_ms": 4500,
    "tool_calls": 29,
    "retry_count": 1,
    "correlation_id": "agt-m-4"
  }
}
```
