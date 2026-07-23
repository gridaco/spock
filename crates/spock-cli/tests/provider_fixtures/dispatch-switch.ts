    case "SetLike":
      return {
        request,
        operation: {
          kind: "set_like",
          post: keyText(requiredField(fields, "post"), POST_ID_TYPE),
          liked: boolValue(requiredField(fields, "liked")),
        },
      };
    case "SetSave":
      return {
        request,
        operation: {
          kind: "set_save",
          post: keyText(requiredField(fields, "post"), POST_ID_TYPE),
          saved: boolValue(requiredField(fields, "saved")),
        },
      };
    case "LoadMore":
      return { request, operation: { kind: "load_more" } };
    case "ReloadFeed":
      return { request, operation: { kind: "reload_feed" } };
    case "SetFollow":
      return {
        request,
        operation: {
          kind: "set_follow",
          user: keyText(requiredField(fields, "user"), USER_ID_TYPE),
          following: boolValue(requiredField(fields, "following")),
        },
      };
    case "AddComment":
      return {
        request,
        operation: {
          kind: "add_comment",
          post: keyText(requiredField(fields, "post"), POST_ID_TYPE),
          body: textValue(requiredField(fields, "body")),
        },
      };
    case "SearchPeople":
      return {
        request,
        operation: {
          kind: "search_people",
          query: textValue(requiredField(fields, "query")),
        },
      };
    case "ChooseImage":
      return { request, operation: { kind: "choose_image_request" } };
    case "PublishImage":
      return {
        request,
        operation: {
          kind: "publish_image_request",
          object: textValue(requiredField(fields, "object")),
          caption: textValue(requiredField(fields, "caption")),
          alt: textValue(requiredField(fields, "alt")),
        },
      };
    case "MarkStory":
      return {
        request,
        operation: {
          kind: "mark_story",
          story: keyText(requiredField(fields, "story"), STORY_ID_TYPE),
        },
      };
    default:
      throw new TypeError(`unsupported Instagram mutation \`${mutation}\``);
