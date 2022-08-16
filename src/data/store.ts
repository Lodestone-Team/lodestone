import { configureStore } from '@reduxjs/toolkit'
import InstanceListReducer from './InstanceList'
import ClientInfoReducer from './ClientInfo'

export const store = configureStore({
  reducer: {
    instanceList: InstanceListReducer,
    clientInfo: ClientInfoReducer,
  },
})

// Infer the `RootState` and `AppDispatch` types from the store itself
export type RootState = ReturnType<typeof store.getState>
// Inferred type: {posts: PostsState, comments: CommentsState, users: UsersState}
export type AppDispatch = typeof store.dispatch
