/* eslint-disable */
import * as types from './graphql';



/**
 * Map of all GraphQL operations in the project.
 *
 * This map has several performance disadvantages:
 * 1. It is not tree-shakeable, so it will include all operations in the project.
 * 2. It is not minifiable, so the string of a GraphQL query will be multiple times inside the bundle.
 * 3. It does not support dead code elimination, so it will add unused operations.
 *
 * Therefore it is highly recommended to use the babel or swc plugin for production.
 * Learn more about it here: https://the-guild.dev/graphql/codegen/plugins/presets/preset-client#reducing-bundle-size
 */
type Documents = {
    "\n  query Feed {\n    user {\n      username\n      post_by_author {\n        caption\n        like_by_post {\n          user {\n            username\n          }\n        }\n      }\n    }\n  }\n": typeof types.FeedDocument,
    "\n  mutation InsertUser($object: user_insert_input!) {\n    insert_user_one(object: $object) {\n      id\n      username\n      joined_at\n    }\n  }\n": typeof types.InsertUserDocument,
    "\n  mutation UpdateBio($id: uuid!, $set: user_set_input!) {\n    update_user_by_pk(pk_columns: { id: $id }, _set: $set) {\n      username\n      bio\n    }\n  }\n": typeof types.UpdateBioDocument,
    "\n  mutation DeleteUser($id: uuid!) {\n    delete_user_by_pk(id: $id) {\n      username\n    }\n  }\n": typeof types.DeleteUserDocument,
    "\n  query Users {\n    user(limit: 200) {\n      id\n      username\n    }\n  }\n": typeof types.UsersDocument,
};
const documents: Documents = {
    "\n  query Feed {\n    user {\n      username\n      post_by_author {\n        caption\n        like_by_post {\n          user {\n            username\n          }\n        }\n      }\n    }\n  }\n": types.FeedDocument,
    "\n  mutation InsertUser($object: user_insert_input!) {\n    insert_user_one(object: $object) {\n      id\n      username\n      joined_at\n    }\n  }\n": types.InsertUserDocument,
    "\n  mutation UpdateBio($id: uuid!, $set: user_set_input!) {\n    update_user_by_pk(pk_columns: { id: $id }, _set: $set) {\n      username\n      bio\n    }\n  }\n": types.UpdateBioDocument,
    "\n  mutation DeleteUser($id: uuid!) {\n    delete_user_by_pk(id: $id) {\n      username\n    }\n  }\n": types.DeleteUserDocument,
    "\n  query Users {\n    user(limit: 200) {\n      id\n      username\n    }\n  }\n": types.UsersDocument,
};

/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Feed {\n    user {\n      username\n      post_by_author {\n        caption\n        like_by_post {\n          user {\n            username\n          }\n        }\n      }\n    }\n  }\n"): typeof import('./graphql').FeedDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation InsertUser($object: user_insert_input!) {\n    insert_user_one(object: $object) {\n      id\n      username\n      joined_at\n    }\n  }\n"): typeof import('./graphql').InsertUserDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation UpdateBio($id: uuid!, $set: user_set_input!) {\n    update_user_by_pk(pk_columns: { id: $id }, _set: $set) {\n      username\n      bio\n    }\n  }\n"): typeof import('./graphql').UpdateBioDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  mutation DeleteUser($id: uuid!) {\n    delete_user_by_pk(id: $id) {\n      username\n    }\n  }\n"): typeof import('./graphql').DeleteUserDocument;
/**
 * The graphql function is used to parse GraphQL queries into a document that can be used by GraphQL clients.
 */
export function graphql(source: "\n  query Users {\n    user(limit: 200) {\n      id\n      username\n    }\n  }\n"): typeof import('./graphql').UsersDocument;


export function graphql(source: string) {
  return (documents as any)[source] ?? {};
}
