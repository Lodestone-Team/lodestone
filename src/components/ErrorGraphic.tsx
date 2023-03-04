import { FontAwesomeIcon } from "@fortawesome/react-fontawesome"
import { IconDefinition } from "@fortawesome/free-solid-svg-icons"
// import * as fileIcon from "@fortawesome/free-solid-svg-icons"

interface ErrorGraphicProps {
  iconProp: IconDefinition;
  message: string;
  message2?: string;
  className?: string;
  iconClassName: string;
  messageClassName: string
}
export default function ErrorGraphic({ className, iconProp, iconClassName, message, message2, messageClassName }: ErrorGraphicProps) {
    return (
      <div className={`flex h-full w-full grow flex-col items-center justify-center gap-4 bg-gray-800 ${className}`}>
        <FontAwesomeIcon
          icon={iconProp}
          className={`text-title ${iconClassName}`}
        />
        <p className={`text-center text-h3 font-medium ${messageClassName}`}>
          {message}
        </p>
        {message2 && (
          <p className={`text-center text-h3 font-medium ${messageClassName}`}>
            {message2}
          </p>
        )}
      </div>
    )
}
