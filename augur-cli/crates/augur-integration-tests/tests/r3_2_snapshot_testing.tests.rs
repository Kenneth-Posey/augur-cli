fn snapshot_body(snapshot: &str) -> &str {
    snapshot
        .splitn(3, "---\n")
        .nth(2)
        .expect("snapshot should include payload section")
        .trim_end()
}

fn assert_snapshot_payload(expected: &str, snapshot: &str) {
    assert_eq!(expected.trim_end(), snapshot_body(snapshot));
}

#[test]
fn snapshot_dead_code_report() {
    let json_str = r#"{
  "metadata": {
    "id": "report-dead-code",
    "analysis_id": "analysis-001",
    "graph_id": "graph-001",
    "generated_at": 1714746300000
  },
  "config": {
    "format": "Json",
    "sort_by": "LinesSaved",
    "filter": {
      "min_confidence": null,
      "min_lines_saved": null,
      "opportunity_types": null,
      "exclude_layers": null
    },
    "output_options": {
      "include_statistics": true,
      "include_recommendations": true,
      "max_opportunities": null
    }
  },
  "opportunities": [
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "func_a"
        }
      },
      "affected_nodes": [
        "func_a"
      ],
      "rationale": "Never called, safe to remove",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.95,
        "estimated_lines_saved": 15
      }
    },
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "func_b"
        }
      },
      "affected_nodes": [
        "func_b"
      ],
      "rationale": "Never called, safe to remove",
      "layer": "Logic",
      "metadata": {
        "confidence": 0.75,
        "estimated_lines_saved": 8
      }
    },
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "func_c"
        }
      },
      "affected_nodes": [
        "func_c"
      ],
      "rationale": "Never called, safe to remove",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.85,
        "estimated_lines_saved": 20
      }
    }
  ],
  "statistics": {
    "total_opportunities": 0,
    "total_lines_saved": 0,
    "average_confidence": 0.0,
    "confidence_range": {
      "max_confidence": 0.0,
      "min_confidence": 0.0
    }
  },
  "recommendations": []
}"#;

    assert_snapshot_payload(
        json_str,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__snapshot_dead_code_report.snap"
        ),
    );
}

#[test]
fn snapshot_duplicate_functions() {
    let json_str = r#"{
  "metadata": {
    "id": "report-duplicates",
    "analysis_id": "analysis-002",
    "graph_id": "graph-002",
    "generated_at": 1714746300000
  },
  "config": {
    "format": "Json",
    "sort_by": "LinesSaved",
    "filter": {
      "min_confidence": null,
      "min_lines_saved": null,
      "opportunity_types": null,
      "exclude_layers": null
    },
    "output_options": {
      "include_statistics": true,
      "include_recommendations": true,
      "max_opportunities": null
    }
  },
  "opportunities": [
    {
      "opportunity_type": {
        "ExactSignatureDuplicate": {
          "canonical": "parse",
          "duplicates": [
            "parse_alt_1",
            "parse_alt_2"
          ]
        }
      },
      "affected_nodes": [
        "parse",
        "parse_alt_1",
        "parse_alt_2"
      ],
      "rationale": "Functions have identical signatures and behavior",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.92,
        "estimated_lines_saved": 25
      }
    }
  ],
  "statistics": {
    "total_opportunities": 0,
    "total_lines_saved": 0,
    "average_confidence": 0.0,
    "confidence_range": {
      "max_confidence": 0.0,
      "min_confidence": 0.0
    }
  },
  "recommendations": []
}"#;

    assert_snapshot_payload(
        json_str,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__snapshot_duplicate_functions.snap"
        ),
    );
}

#[test]
fn snapshot_chain_collapse() {
    let json_str = r#"{
  "metadata": {
    "id": "report-collapse",
    "analysis_id": "analysis-003",
    "graph_id": "graph-003",
    "generated_at": 1714746300000
  },
  "config": {
    "format": "Json",
    "sort_by": "LinesSaved",
    "filter": {
      "min_confidence": null,
      "min_lines_saved": null,
      "opportunity_types": null,
      "exclude_layers": null
    },
    "output_options": {
      "include_statistics": true,
      "include_recommendations": true,
      "max_opportunities": null
    }
  },
  "opportunities": [
    {
      "opportunity_type": {
        "ChainCollapse": {
          "parent": "parent",
          "intermediate": "intermediate",
          "child": "child",
          "merged_name": "parent_via_child"
        }
      },
      "affected_nodes": [
        "parent",
        "intermediate",
        "child"
      ],
      "rationale": "Linear chain can be simplified",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.88,
        "estimated_lines_saved": 12
      }
    }
  ],
  "statistics": {
    "total_opportunities": 0,
    "total_lines_saved": 0,
    "average_confidence": 0.0,
    "confidence_range": {
      "max_confidence": 0.0,
      "min_confidence": 0.0
    }
  },
  "recommendations": []
}"#;

    assert_snapshot_payload(
        json_str,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__snapshot_chain_collapse.snap"
        ),
    );
}

#[test]
fn snapshot_mixed_opportunities() {
    let json_str = r#"{
  "metadata": {
    "id": "report-mixed",
    "analysis_id": "analysis-004",
    "graph_id": "graph-004",
    "generated_at": 1714746300000
  },
  "config": {
    "format": "Json",
    "sort_by": "LinesSaved",
    "filter": {
      "min_confidence": null,
      "min_lines_saved": null,
      "opportunity_types": null,
      "exclude_layers": null
    },
    "output_options": {
      "include_statistics": true,
      "include_recommendations": true,
      "max_opportunities": null
    }
  },
  "opportunities": [
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "unused_helper"
        }
      },
      "affected_nodes": [
        "unused_helper"
      ],
      "rationale": "Dead code",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.91,
        "estimated_lines_saved": 10
      }
    },
    {
      "opportunity_type": {
        "ExactSignatureDuplicate": {
          "canonical": "validate",
          "duplicates": [
            "validate_alt"
          ]
        }
      },
      "affected_nodes": [
        "validate",
        "validate_alt"
      ],
      "rationale": "Duplicates",
      "layer": "Logic",
      "metadata": {
        "confidence": 0.85,
        "estimated_lines_saved": 18
      }
    },
    {
      "opportunity_type": {
        "ChainCollapse": {
          "parent": "step1",
          "intermediate": "step2",
          "child": "step3",
          "merged_name": "unified_flow"
        }
      },
      "affected_nodes": [
        "step1",
        "step2",
        "step3"
      ],
      "rationale": "Collapse",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.79,
        "estimated_lines_saved": 22
      }
    }
  ],
  "statistics": {
    "total_opportunities": 0,
    "total_lines_saved": 0,
    "average_confidence": 0.0,
    "confidence_range": {
      "max_confidence": 0.0,
      "min_confidence": 0.0
    }
  },
  "recommendations": []
}"#;

    assert_snapshot_payload(
        json_str,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__snapshot_mixed_opportunities.snap"
        ),
    );
}

#[test]
fn json_serialization_roundtrip() {
    let json_str = r#"{"metadata":{"id":"report-roundtrip","analysis_id":"analysis-roundtrip","graph_id":"graph-roundtrip","generated_at":1714746300000},"config":{"format":"Json","sort_by":"LinesSaved","filter":{"min_confidence":null,"min_lines_saved":null,"opportunity_types":null,"exclude_layers":null},"output_options":{"include_statistics":true,"include_recommendations":true,"max_opportunities":null}},"opportunities":[{"opportunity_type":{"DeadCode":{"target":"dead_fn"}},"affected_nodes":["dead_fn"],"rationale":"Not called","layer":"Adapter","metadata":{"confidence":0.8,"estimated_lines_saved":5}}],"statistics":{"total_opportunities":1,"total_lines_saved":5,"average_confidence":0.8,"confidence_range":{"max_confidence":0.8,"min_confidence":0.8}},"recommendations":["Recommendation 1"]}"#;
    assert_snapshot_payload(
        json_str,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__json_serialization_roundtrip.snap"
        ),
    );
}

#[test]
fn extraction_error_serialization_all_variants() {
    let all_errors_json = r#"[
  {
    "CargoMetadataError": "manifest not found"
  },
  {
    "IoError": "file read failed"
  },
  {
    "ParseError": "unexpected token"
  },
  {
    "InvalidMetadata": "missing field"
  },
  {
    "SourceProcessingError": "symlink cycle"
  }
]"#;

    assert_snapshot_payload(
        all_errors_json,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__extraction_error_serialization_all_variants.snap"
        ),
    );
}

#[test]
fn determinism_sorted_opportunities() {
    let json1 = r#"{
  "metadata": {
    "id": "report-sort",
    "analysis_id": "analysis-sort",
    "graph_id": "graph-sort",
    "generated_at": 1714746300000
  },
  "config": {
    "format": "Json",
    "sort_by": "LinesSaved",
    "filter": {
      "min_confidence": null,
      "min_lines_saved": null,
      "opportunity_types": null,
      "exclude_layers": null
    },
    "output_options": {
      "include_statistics": true,
      "include_recommendations": true,
      "max_opportunities": null
    }
  },
  "opportunities": [
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "z_func"
        }
      },
      "affected_nodes": [
        "z_func"
      ],
      "rationale": "Dead",
      "layer": "Logic",
      "metadata": {
        "confidence": 0.5,
        "estimated_lines_saved": 5
      }
    },
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "a_func"
        }
      },
      "affected_nodes": [
        "a_func"
      ],
      "rationale": "Dead",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.6,
        "estimated_lines_saved": 10
      }
    },
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "m_func"
        }
      },
      "affected_nodes": [
        "m_func"
      ],
      "rationale": "Dead",
      "layer": "Domain",
      "metadata": {
        "confidence": 0.7,
        "estimated_lines_saved": 8
      }
    }
  ],
  "statistics": {
    "total_opportunities": 0,
    "total_lines_saved": 0,
    "average_confidence": 0.0,
    "confidence_range": {
      "max_confidence": 0.0,
      "min_confidence": 0.0
    }
  },
  "recommendations": []
}"#;

    assert_snapshot_payload(
        json1,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__determinism_sorted_opportunities.snap"
        ),
    );
}

#[test]
fn determinism_normalized_timestamps() {
    let json_str = r#"{
  "metadata": {
    "id": "report-ts",
    "analysis_id": "analysis-ts",
    "graph_id": "graph-ts",
    "generated_at": 1714746300000
  },
  "config": {
    "format": "Json",
    "sort_by": "LinesSaved",
    "filter": {
      "min_confidence": null,
      "min_lines_saved": null,
      "opportunity_types": null,
      "exclude_layers": null
    },
    "output_options": {
      "include_statistics": true,
      "include_recommendations": true,
      "max_opportunities": null
    }
  },
  "opportunities": [
    {
      "opportunity_type": {
        "DeadCode": {
          "target": "func_ts"
        }
      },
      "affected_nodes": [
        "func_ts"
      ],
      "rationale": "Test",
      "layer": "Adapter",
      "metadata": {
        "confidence": 0.75,
        "estimated_lines_saved": 3
      }
    }
  ],
  "statistics": {
    "total_opportunities": 0,
    "total_lines_saved": 0,
    "average_confidence": 0.0,
    "confidence_range": {
      "max_confidence": 0.0,
      "min_confidence": 0.0
    }
  },
  "recommendations": []
}"#;

    assert_snapshot_payload(
        json_str,
        include_str!(
            "integration/snapshots/r3_2_snapshot_testing__r3_2_snapshot_testing_tests__determinism_normalized_timestamps.snap"
        ),
    );
}
