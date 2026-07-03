import type { WrapperChainItem } from '../../utils/derivePipelineDetail';

export interface WrapperChainProps {
  items: WrapperChainItem[];
  emptyMessage: string;
}

export function WrapperChain({ items, emptyMessage }: WrapperChainProps) {
  if (items.length === 0) {
    return <p className="crosshook-wrapper-chain__empty">{emptyMessage}</p>;
  }

  return (
    <ol className="crosshook-wrapper-chain">
      {items.map((item) => (
        <li className="crosshook-wrapper-chain__item" data-state={item.active ? 'active' : 'inactive'} key={item.id}>
          <code className="crosshook-wrapper-chain__token">{item.token}</code>
          <span className="crosshook-wrapper-chain__badge">{item.active ? 'Active' : 'Inactive'}</span>
          <span className="crosshook-wrapper-chain__reason">{item.reason}</span>
          {item.foldedInto ? (
            <span className="crosshook-wrapper-chain__folded">folded into {item.foldedInto}</span>
          ) : null}
          {item.detail ? <span className="crosshook-wrapper-chain__detail">{item.detail}</span> : null}
        </li>
      ))}
    </ol>
  );
}

export default WrapperChain;
