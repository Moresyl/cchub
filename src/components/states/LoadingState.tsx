interface LoadingStateProps {
  label?: string;
}

export default function LoadingState({ label }: LoadingStateProps) {
  return (
    <div className="loading-center">
      <div className="spinner" aria-hidden="true" />
      {label ? <div className="state-copy">{label}</div> : null}
    </div>
  );
}
