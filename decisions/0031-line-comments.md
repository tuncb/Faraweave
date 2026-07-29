# Line comments

The tokenizer represents `#` comments as trivia tokens with empty spelling, preserving byte spans without retaining comment text. Line endings remain separate tokens so existing newline-sensitive grammar and diagnostics keep their behavior. Both recursive and compact parser paths share the same trivia classification.
