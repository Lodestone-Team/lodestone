import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { IconDefinition } from "@fortawesome/free-solid-svg-icons"
import { cn } from "utils/util";

interface ErrorGraphicProps {
  icon: IconDefinition;
  message: string;
  message2?: string;
  className?: string;
  iconClassName: string;
  messageClassName: string
}

export default function ErrorGraphic({ className, icon, iconClassName, message, message2, messageClassName }: ErrorGraphicProps) {
    return (
      <div className={cn('flex', 'h-full', 'w-full', 'grow', 'flex-col', 'items-center', 'justify-center', 'gap-4', 'bg-gray-800', className)}>
        <FontAwesomeIcon
          icon={icon}
          className={`text-title ${iconClassName}`}
        />
        <p className={cn('text-center', 'text-h3', 'font-medium', messageClassName)}>
          {message}
        </p>
        {message2 && (
          <p className={cn('text-center', 'text-h3', 'font-medium', messageClassName)}>
            {message2}
          </p>
        )}
      </div>
    )
}
