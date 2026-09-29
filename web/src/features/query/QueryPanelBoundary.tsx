import { Component } from "react";
import type { ErrorInfo, ReactNode } from "react";
import { queryStore } from "../../app/store";

/**
 * Keeps a failure of the requirement editor inside the query pane. The query
 * is persisted, so requirements the editor cannot draw would come back on
 * every reload; the fallback says what failed and offers the way out.
 */
export class QueryPanelBoundary extends Component<
  { children: ReactNode },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("The query panel failed", error, info.componentStack);
  }

  private clearRequirements = () => {
    queryStore.setState((state) => ({
      ...state,
      requirements: [],
      arcaneResin: undefined,
      arcaneResinFilter: undefined,
    }));
    this.setState({ error: null });
  };

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;
    return (
      <div className="d1-pane-body">
        <div className="d1-banner d1-banner-error d1-query-failure" role="alert">
          <p>The requirements could not be shown: {error.message}</p>
          <button type="button" className="d1-btn" onClick={this.clearRequirements}>
            Clear requirements
          </button>
        </div>
      </div>
    );
  }
}
