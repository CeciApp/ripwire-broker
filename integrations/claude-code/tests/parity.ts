// Written by tests/hooks.rs::plan_decisions_are_exported_for_the_mod (hook::plan's
// decisions for recorded events); do not edit. RIPWIRE_BROKER_WRITE_PARITY=1 writes it again.
export const PARITY = [
  {
    "every_prompt": false,
    "name": "only the first prompt",
    "steps": [
      {
        "decision": {
          "ask": "context_for_task",
          "budget_tokens": 1500,
          "task": "Edit a.txt so that it contains exactly: hello. Then reply done."
        },
        "event": {
          "cwd": "/work",
          "hook_event_name": "UserPromptSubmit",
          "permission_mode": "acceptEdits",
          "prompt": "Edit a.txt so that it contains exactly: hello. Then reply done.",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10000
      },
      {
        "decision": {
          "ask": null
        },
        "event": {
          "cwd": "/work",
          "hook_event_name": "UserPromptSubmit",
          "permission_mode": "acceptEdits",
          "prompt": "and now the tests",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 20000
      }
    ]
  },
  {
    "every_prompt": true,
    "name": "every prompt",
    "steps": [
      {
        "decision": {
          "ask": "context_for_task",
          "budget_tokens": 1500,
          "task": "Edit a.txt so that it contains exactly: hello. Then reply done."
        },
        "event": {
          "cwd": "/work",
          "hook_event_name": "UserPromptSubmit",
          "permission_mode": "acceptEdits",
          "prompt": "Edit a.txt so that it contains exactly: hello. Then reply done.",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10000
      },
      {
        "decision": {
          "ask": "context_for_task",
          "budget_tokens": 1500,
          "task": "and now the tests"
        },
        "event": {
          "cwd": "/work",
          "hook_event_name": "UserPromptSubmit",
          "permission_mode": "acceptEdits",
          "prompt": "and now the tests",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 20000
      }
    ]
  },
  {
    "every_prompt": true,
    "name": "a pause and a resume",
    "steps": [
      {
        "decision": {
          "ask": null
        },
        "event": {
          "cwd": "/work",
          "hook_event_name": "UserPromptSubmit",
          "permission_mode": "acceptEdits",
          "prompt": "#ripwire-off",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10000
      },
      {
        "decision": {
          "ask": null
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 2,
          "hook_event_name": "PostToolUse",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "tool_input": {
            "file_path": "/work/a.txt",
            "new_string": "hello.",
            "old_string": "hi\n",
            "replace_all": false
          },
          "tool_name": "Edit",
          "tool_response": {
            "filePath": "/work/a.txt",
            "newString": "hello.",
            "oldString": "hi\n",
            "originalFile": "hi\n",
            "replaceAll": false,
            "structuredPatch": [
              {
                "lines": [
                  "-hi",
                  "+hello.",
                  "\\ No newline at end of file"
                ],
                "newLines": 1,
                "newStart": 1,
                "oldLines": 1,
                "oldStart": 1
              }
            ],
            "userModified": false
          },
          "tool_use_id": "toolu_019vFfWozoNwMkYBh8Maw5d3",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 11000
      },
      {
        "decision": {
          "ask": null
        },
        "event": {
          "background_tasks": [],
          "cwd": "/work",
          "hook_event_name": "Stop",
          "last_assistant_message": "done.",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_crons": [],
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "stop_hook_active": false,
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 12000
      },
      {
        "decision": {
          "ask": "context_for_task",
          "budget_tokens": 1500,
          "task": "fix it"
        },
        "event": {
          "cwd": "/work",
          "hook_event_name": "UserPromptSubmit",
          "permission_mode": "acceptEdits",
          "prompt": "fix it #ripwire-on",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 13000
      },
      {
        "decision": {
          "ask": "context_after_edit",
          "budget_tokens": 800,
          "files": [
            "a.txt"
          ]
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 2,
          "hook_event_name": "PostToolUse",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "tool_input": {
            "file_path": "/work/a.txt",
            "new_string": "hello.",
            "old_string": "hi\n",
            "replace_all": false
          },
          "tool_name": "Edit",
          "tool_response": {
            "filePath": "/work/a.txt",
            "newString": "hello.",
            "oldString": "hi\n",
            "originalFile": "hi\n",
            "replaceAll": false,
            "structuredPatch": [
              {
                "lines": [
                  "-hi",
                  "+hello.",
                  "\\ No newline at end of file"
                ],
                "newLines": 1,
                "newStart": 1,
                "oldLines": 1,
                "oldStart": 1
              }
            ],
            "userModified": false
          },
          "tool_use_id": "toolu_019vFfWozoNwMkYBh8Maw5d3",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 14000
      }
    ]
  },
  {
    "every_prompt": false,
    "name": "a burst of edits",
    "steps": [
      {
        "decision": {
          "ask": "context_after_edit",
          "budget_tokens": 800,
          "files": [
            "a.txt"
          ]
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 2,
          "hook_event_name": "PostToolUse",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "tool_input": {
            "file_path": "/work/a.txt",
            "new_string": "hello.",
            "old_string": "hi\n",
            "replace_all": false
          },
          "tool_name": "Edit",
          "tool_response": {
            "filePath": "/work/a.txt",
            "newString": "hello.",
            "oldString": "hi\n",
            "originalFile": "hi\n",
            "replaceAll": false,
            "structuredPatch": [
              {
                "lines": [
                  "-hi",
                  "+hello.",
                  "\\ No newline at end of file"
                ],
                "newLines": 1,
                "newStart": 1,
                "oldLines": 1,
                "oldStart": 1
              }
            ],
            "userModified": false
          },
          "tool_use_id": "toolu_019vFfWozoNwMkYBh8Maw5d3",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10000
      },
      {
        "decision": {
          "ask": null
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 2,
          "hook_event_name": "PostToolUse",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "tool_input": {
            "file_path": "/work/b.txt",
            "new_string": "hello.",
            "old_string": "hi\n",
            "replace_all": false
          },
          "tool_name": "Edit",
          "tool_response": {
            "filePath": "/work/a.txt",
            "newString": "hello.",
            "oldString": "hi\n",
            "originalFile": "hi\n",
            "replaceAll": false,
            "structuredPatch": [
              {
                "lines": [
                  "-hi",
                  "+hello.",
                  "\\ No newline at end of file"
                ],
                "newLines": 1,
                "newStart": 1,
                "oldLines": 1,
                "oldStart": 1
              }
            ],
            "userModified": false
          },
          "tool_use_id": "toolu_019vFfWozoNwMkYBh8Maw5d3",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10500
      },
      {
        "decision": {
          "ask": null
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 2,
          "hook_event_name": "PostToolUse",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "tool_input": {
            "file_path": "/work/a.txt",
            "new_string": "hello.",
            "old_string": "hi\n",
            "replace_all": false
          },
          "tool_name": "Edit",
          "tool_response": {
            "filePath": "/work/a.txt",
            "newString": "hello.",
            "oldString": "hi\n",
            "originalFile": "hi\n",
            "replaceAll": false,
            "structuredPatch": [
              {
                "lines": [
                  "-hi",
                  "+hello.",
                  "\\ No newline at end of file"
                ],
                "newLines": 1,
                "newStart": 1,
                "oldLines": 1,
                "oldStart": 1
              }
            ],
            "userModified": false
          },
          "tool_use_id": "toolu_019vFfWozoNwMkYBh8Maw5d3",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10999
      },
      {
        "decision": {
          "ask": "context_after_edit",
          "budget_tokens": 800,
          "files": [
            "c.txt",
            "b.txt",
            "a.txt"
          ]
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 2,
          "hook_event_name": "PostToolUse",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "tool_input": {
            "file_path": "/work/c.txt",
            "new_string": "hello.",
            "old_string": "hi\n",
            "replace_all": false
          },
          "tool_name": "Edit",
          "tool_response": {
            "filePath": "/work/a.txt",
            "newString": "hello.",
            "oldString": "hi\n",
            "originalFile": "hi\n",
            "replaceAll": false,
            "structuredPatch": [
              {
                "lines": [
                  "-hi",
                  "+hello.",
                  "\\ No newline at end of file"
                ],
                "newLines": 1,
                "newStart": 1,
                "oldLines": 1,
                "oldStart": 1
              }
            ],
            "userModified": false
          },
          "tool_use_id": "toolu_019vFfWozoNwMkYBh8Maw5d3",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 11000
      }
    ]
  },
  {
    "every_prompt": false,
    "name": "an edit outside the workspace",
    "steps": [
      {
        "decision": {
          "ask": null
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 2,
          "hook_event_name": "PostToolUse",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "tool_input": {
            "file_path": "/elsewhere/a.txt",
            "new_string": "hello.",
            "old_string": "hi\n",
            "replace_all": false
          },
          "tool_name": "Edit",
          "tool_response": {
            "filePath": "/work/a.txt",
            "newString": "hello.",
            "oldString": "hi\n",
            "originalFile": "hi\n",
            "replaceAll": false,
            "structuredPatch": [
              {
                "lines": [
                  "-hi",
                  "+hello.",
                  "\\ No newline at end of file"
                ],
                "newLines": 1,
                "newStart": 1,
                "oldLines": 1,
                "oldStart": 1
              }
            ],
            "userModified": false
          },
          "tool_use_id": "toolu_019vFfWozoNwMkYBh8Maw5d3",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10000
      }
    ]
  },
  {
    "every_prompt": false,
    "name": "shell commands",
    "steps": [
      {
        "decision": {
          "ask": "context_after_edit",
          "budget_tokens": 800,
          "files": [
            "src/lib.rs"
          ]
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 1071,
          "effort": {
            "level": "medium"
          },
          "hook_event_name": "PostToolUse",
          "permission_mode": "auto",
          "prompt_id": "30776374-33f3-4c0c-891b-84fc9157d129",
          "scratchpad_dir": "__SCRATCHPAD__",
          "session_id": "907cda3c-c8b0-46f0-9302-5dd057c8a510",
          "tool_input": {
            "command": "echo '// x' >> src/lib.rs && tail -n 3 src/lib.rs",
            "description": "Append comment line to lib.rs and show tail"
          },
          "tool_name": "Bash",
          "tool_response": {
            "bashEditDiff": {
              "changedFiles": [
                "/work/src/lib.rs"
              ],
              "files": [
                {
                  "filePath": "/work/src/lib.rs",
                  "hunks": [
                    {
                      "lines": [
                        " pub fn add(a: i32, b: i32) -> i32 { a + b }",
                        " pub fn sub(a: i32, b: i32) -> i32 { a - b }",
                        " pub fn mul(a: i32, b: i32) -> i32 { a * b }",
                        "+// x"
                      ],
                      "newLines": 4,
                      "newStart": 1,
                      "oldLines": 3,
                      "oldStart": 1
                    }
                  ]
                }
              ],
              "moreFiles": 0
            },
            "interrupted": false,
            "isImage": false,
            "noOutputExpected": false,
            "stderr": "",
            "stdout": "pub fn sub(a: i32, b: i32) -> i32 { a - b }\npub fn mul(a: i32, b: i32) -> i32 { a * b }\n// x"
          },
          "tool_use_id": "toolu_01McuBqixSEair7vJPmnZnGX",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10000
      },
      {
        "decision": {
          "ask": "context_after_edit",
          "budget_tokens": 800,
          "files": [
            "gen_1.txt",
            "gen_10.txt",
            "gen_11.txt",
            "gen_12.txt",
            "gen_13.txt",
            "gen_14.txt",
            "gen_15.txt",
            "gen_16.txt",
            "gen_17.txt",
            "gen_18.txt",
            "gen_19.txt",
            "gen_2.txt",
            "gen_20.txt",
            "gen_21.txt",
            "gen_22.txt",
            "gen_23.txt",
            "gen_24.txt",
            "gen_25.txt",
            "gen_26.txt",
            "gen_27.txt",
            "gen_28.txt",
            "gen_29.txt",
            "gen_3.txt",
            "gen_30.txt",
            "gen_31.txt",
            "gen_32.txt",
            "gen_33.txt",
            "gen_34.txt",
            "gen_35.txt",
            "gen_36.txt",
            "gen_37.txt",
            "gen_38.txt",
            "gen_39.txt",
            "gen_4.txt",
            "gen_40.txt",
            "gen_41.txt",
            "gen_42.txt",
            "gen_43.txt",
            "gen_44.txt",
            "gen_45.txt",
            "gen_46.txt",
            "gen_47.txt",
            "gen_48.txt",
            "gen_49.txt",
            "gen_5.txt",
            "gen_50.txt",
            "gen_51.txt",
            "gen_52.txt",
            "gen_53.txt",
            "gen_54.txt"
          ]
        },
        "event": {
          "cwd": "/work",
          "duration_ms": 222,
          "effort": {
            "level": "medium"
          },
          "hook_event_name": "PostToolUse",
          "permission_mode": "auto",
          "prompt_id": "375f1aa1-505b-44a8-8a04-61a056452aa3",
          "scratchpad_dir": "__SCRATCHPAD__",
          "session_id": "dc291498-07f1-46c4-8a58-7f22ac71866c",
          "tool_input": {
            "command": "for i in $(seq 1 60); do echo $i > gen_$i.txt; done; ls gen_*.txt | wc -l",
            "description": "Create 60 numbered text files and count them"
          },
          "tool_name": "Bash",
          "tool_response": {
            "bashEditDiff": {
              "changedFiles": [
                "/work/gen_1.txt",
                "/work/gen_10.txt",
                "/work/gen_11.txt",
                "/work/gen_12.txt",
                "/work/gen_13.txt",
                "/work/gen_14.txt",
                "/work/gen_15.txt",
                "/work/gen_16.txt",
                "/work/gen_17.txt",
                "/work/gen_18.txt",
                "/work/gen_19.txt",
                "/work/gen_2.txt",
                "/work/gen_20.txt",
                "/work/gen_21.txt",
                "/work/gen_22.txt",
                "/work/gen_23.txt",
                "/work/gen_24.txt",
                "/work/gen_25.txt",
                "/work/gen_26.txt",
                "/work/gen_27.txt",
                "/work/gen_28.txt",
                "/work/gen_29.txt",
                "/work/gen_3.txt",
                "/work/gen_30.txt",
                "/work/gen_31.txt",
                "/work/gen_32.txt",
                "/work/gen_33.txt",
                "/work/gen_34.txt",
                "/work/gen_35.txt",
                "/work/gen_36.txt",
                "/work/gen_37.txt",
                "/work/gen_38.txt",
                "/work/gen_39.txt",
                "/work/gen_4.txt",
                "/work/gen_40.txt",
                "/work/gen_41.txt",
                "/work/gen_42.txt",
                "/work/gen_43.txt",
                "/work/gen_44.txt",
                "/work/gen_45.txt",
                "/work/gen_46.txt",
                "/work/gen_47.txt",
                "/work/gen_48.txt",
                "/work/gen_49.txt",
                "/work/gen_5.txt",
                "/work/gen_50.txt",
                "/work/gen_51.txt",
                "/work/gen_52.txt",
                "/work/gen_53.txt",
                "/work/gen_54.txt",
                "/work/gen_55.txt",
                "/work/gen_56.txt",
                "/work/gen_57.txt",
                "/work/gen_58.txt",
                "/work/gen_59.txt",
                "/work/gen_6.txt",
                "/work/gen_60.txt",
                "/work/gen_7.txt",
                "/work/gen_8.txt",
                "/work/gen_9.txt"
              ],
              "files": [
                {
                  "created": true,
                  "filePath": "/work/gen_1.txt",
                  "hunks": [
                    {
                      "lines": [
                        "+1"
                      ],
                      "newLines": 1,
                      "newStart": 1,
                      "oldLines": 0,
                      "oldStart": 0
                    }
                  ]
                },
                {
                  "created": true,
                  "filePath": "/work/gen_10.txt",
                  "hunks": [
                    {
                      "lines": [
                        "+10"
                      ],
                      "newLines": 1,
                      "newStart": 1,
                      "oldLines": 0,
                      "oldStart": 0
                    }
                  ]
                },
                {
                  "created": true,
                  "filePath": "/work/gen_11.txt",
                  "hunks": [
                    {
                      "lines": [
                        "+11"
                      ],
                      "newLines": 1,
                      "newStart": 1,
                      "oldLines": 0,
                      "oldStart": 0
                    }
                  ]
                },
                {
                  "created": true,
                  "filePath": "/work/gen_12.txt",
                  "hunks": [
                    {
                      "lines": [
                        "+12"
                      ],
                      "newLines": 1,
                      "newStart": 1,
                      "oldLines": 0,
                      "oldStart": 0
                    }
                  ]
                },
                {
                  "created": true,
                  "filePath": "/work/gen_13.txt",
                  "hunks": [
                    {
                      "lines": [
                        "+13"
                      ],
                      "newLines": 1,
                      "newStart": 1,
                      "oldLines": 0,
                      "oldStart": 0
                    }
                  ]
                }
              ],
              "moreFiles": 55
            },
            "interrupted": false,
            "isImage": false,
            "noOutputExpected": false,
            "stderr": "",
            "stdout": "      60"
          },
          "tool_use_id": "toolu_017XAmsmWDeTfDVA4MPg7mfb",
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 20000
      }
    ]
  },
  {
    "every_prompt": false,
    "name": "the end of a turn",
    "steps": [
      {
        "decision": {
          "ask": "context_before_finish",
          "looping": false
        },
        "event": {
          "background_tasks": [],
          "cwd": "/work",
          "hook_event_name": "Stop",
          "last_assistant_message": "done.",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_crons": [],
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "stop_hook_active": false,
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 10000
      },
      {
        "decision": {
          "ask": "context_before_finish",
          "looping": true
        },
        "event": {
          "background_tasks": [],
          "cwd": "/work",
          "hook_event_name": "Stop",
          "last_assistant_message": "done.",
          "permission_mode": "acceptEdits",
          "prompt_id": "f8f76ebe-b169-429e-bca4-4967abb8ca7f",
          "session_crons": [],
          "session_id": "b24451d5-7348-4ffd-81b7-354ef6a544fb",
          "stop_hook_active": true,
          "transcript_path": "__TRANSCRIPT__"
        },
        "now_ms": 20000
      }
    ]
  }
] as const
