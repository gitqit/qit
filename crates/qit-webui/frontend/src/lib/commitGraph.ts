import type { CommitHistoryNode } from './types'

export interface CommitGraphRow {
  id: string
  lane: number
  activeBefore: number[]
  activeAfter: number[]
  parentLanes: number[]
}

export interface CommitGraphLayout {
  rows: CommitGraphRow[]
  laneCount: number
}

function firstOpenLane(lanes: Array<string | null>) {
  const lane = lanes.findIndex((value) => value === null)
  return lane === -1 ? lanes.length : lane
}

function trimTrailingEmptyLanes(lanes: Array<string | null>) {
  while (lanes.length > 0 && lanes[lanes.length - 1] === null) {
    lanes.pop()
  }
}

export function buildCommitGraph(commits: CommitHistoryNode[]): CommitGraphLayout {
  const rows: CommitGraphRow[] = []
  let lanes: Array<string | null> = []
  let laneCount = 1

  for (const commit of commits) {
    let lane = lanes.indexOf(commit.id)
    if (lane === -1) {
      lane = firstOpenLane(lanes)
      lanes[lane] = commit.id
    }

    const activeBefore = lanes.flatMap((value, index) => (value ? [index] : []))
    const nextLanes = [...lanes]
    const parentLanes: number[] = []

    for (let index = 0; index < nextLanes.length; index += 1) {
      if (index !== lane && nextLanes[index] === commit.id) {
        nextLanes[index] = null
      }
    }

    if (commit.parents.length === 0) {
      nextLanes[lane] = null
    } else {
      const [firstParent, ...restParents] = commit.parents
      nextLanes[lane] = firstParent
      parentLanes.push(lane)

      for (const parent of restParents) {
        let parentLane = nextLanes.indexOf(parent)
        if (parentLane === -1) {
          parentLane = firstOpenLane(nextLanes)
          nextLanes[parentLane] = parent
        }
        parentLanes.push(parentLane)
      }
    }

    const activeAfter = nextLanes.flatMap((value, index) => (value ? [index] : []))
    laneCount = Math.max(
      laneCount,
      lane + 1,
      ...parentLanes.map((value) => value + 1),
      ...activeBefore.map((value) => value + 1),
      ...activeAfter.map((value) => value + 1),
    )

    rows.push({
      id: commit.id,
      lane,
      activeBefore,
      activeAfter,
      parentLanes,
    })

    trimTrailingEmptyLanes(nextLanes)
    lanes = nextLanes
  }

  return { rows, laneCount }
}
