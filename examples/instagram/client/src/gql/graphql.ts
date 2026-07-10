/* eslint-disable */
/** Internal type. DO NOT USE DIRECTLY. */
type Exact<T extends { [key: string]: unknown }> = { [K in keyof T]: T[K] };
/** Internal type. DO NOT USE DIRECTLY. */
export type Incremental<T> = T | { [P in keyof T]?: P extends ' $fragmentName' | '__typename' ? T[P] : never };
import { DocumentTypeDecoration } from '@graphql-typed-document-node/core';
export type User_Insert_Input = {
  bio?: string | null | undefined;
  full_name?: string | null | undefined;
  id?: string | null | undefined;
  joined_at?: string | null | undefined;
  username?: string | null | undefined;
};

export type User_Set_Input = {
  bio?: string | null | undefined;
  full_name?: string | null | undefined;
  joined_at?: string | null | undefined;
  username?: string | null | undefined;
};

export type FeedQueryVariables = Exact<{ [key: string]: never; }>;


export type FeedQuery = { user: Array<{ username: string, post_by_author: Array<{ caption: string | null, like_by_post: Array<{ user: { username: string } }> }> }> };

export type InsertUserMutationVariables = Exact<{
  object: User_Insert_Input;
}>;


export type InsertUserMutation = { insert_user_one: { id: string, username: string, joined_at: string } };

export type UpdateBioMutationVariables = Exact<{
  id: string;
  set: User_Set_Input;
}>;


export type UpdateBioMutation = { update_user_by_pk: { username: string, bio: string | null } };

export type DeleteUserMutationVariables = Exact<{
  id: string;
}>;


export type DeleteUserMutation = { delete_user_by_pk: { username: string } };

export type UsersQueryVariables = Exact<{ [key: string]: never; }>;


export type UsersQuery = { user: Array<{ id: string, username: string }> };

export class TypedDocumentString<TResult, TVariables>
  extends String
  implements DocumentTypeDecoration<TResult, TVariables>
{
  __apiType?: NonNullable<DocumentTypeDecoration<TResult, TVariables>['__apiType']>;
  private value: string;
  public __meta__?: Record<string, any> | undefined;

  constructor(value: string, __meta__?: Record<string, any> | undefined) {
    super(value);
    this.value = value;
    this.__meta__ = __meta__;
  }

  override toString(): string & DocumentTypeDecoration<TResult, TVariables> {
    return this.value;
  }
}

export const FeedDocument = new TypedDocumentString(`
    query Feed {
  user {
    username
    post_by_author {
      caption
      like_by_post {
        user {
          username
        }
      }
    }
  }
}
    `) as unknown as TypedDocumentString<FeedQuery, FeedQueryVariables>;
export const InsertUserDocument = new TypedDocumentString(`
    mutation InsertUser($object: user_insert_input!) {
  insert_user_one(object: $object) {
    id
    username
    joined_at
  }
}
    `) as unknown as TypedDocumentString<InsertUserMutation, InsertUserMutationVariables>;
export const UpdateBioDocument = new TypedDocumentString(`
    mutation UpdateBio($id: uuid!, $set: user_set_input!) {
  update_user_by_pk(pk_columns: {id: $id}, _set: $set) {
    username
    bio
  }
}
    `) as unknown as TypedDocumentString<UpdateBioMutation, UpdateBioMutationVariables>;
export const DeleteUserDocument = new TypedDocumentString(`
    mutation DeleteUser($id: uuid!) {
  delete_user_by_pk(id: $id) {
    username
  }
}
    `) as unknown as TypedDocumentString<DeleteUserMutation, DeleteUserMutationVariables>;
export const UsersDocument = new TypedDocumentString(`
    query Users {
  user(limit: 200) {
    id
    username
  }
}
    `) as unknown as TypedDocumentString<UsersQuery, UsersQueryVariables>;